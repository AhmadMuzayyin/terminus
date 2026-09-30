package org.terminus.mobile.ssh

import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.util.EnumSet
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.sftp.FileMode
import net.schmizz.sshj.sftp.OpenMode
import net.schmizz.sshj.sftp.Response
import net.schmizz.sshj.sftp.SFTPClient
import net.schmizz.sshj.sftp.SFTPException

/** Jenis entri di server. [Link] = symlink (tujuannya baru diketahui lewat [SftpChannel.isDirectory]). */
enum class RemoteKind { Directory, File, Link }

data class RemoteEntry(val name: String, val path: String, val kind: RemoteKind, val size: Long, val modifiedEpochSeconds: Long)

/** Kegagalan operasi SFTP, pesannya siap tampil. */
class SftpError(message: String, cause: Throwable? = null) : IOException(message, cause)

/** Transfer dibatalkan user. */
class TransferCancelled : IOException("Transfer dibatalkan.")

/**
 * Satu koneksi SFTP (sshj). SEMUA fungsi BLOKING (jaringan) — panggil di
 * Dispatchers.IO. Path selalu absolut (`/home/user/x`).
 */
class SftpChannel internal constructor(private val client: SSHClient, private val sftp: SFTPClient) {

    val isConnected: Boolean get() = client.isConnected && client.isAuthenticated

    /** Folder awal = home user di server. */
    fun home(): String = call { sftp.canonicalize(".") }

    fun list(path: String): List<RemoteEntry> = call {
        sftp.ls(path)
            .filter { it.name != "." && it.name != ".." }
            .map { info ->
                val attrs = info.attributes
                val kind = when (attrs.type) {
                    FileMode.Type.DIRECTORY -> RemoteKind.Directory
                    FileMode.Type.SYMLINK -> RemoteKind.Link
                    else -> RemoteKind.File
                }
                RemoteEntry(info.name, info.path, kind, attrs.size, attrs.mtime)
            }
    }

    /** Mengikuti symlink. */
    fun isDirectory(path: String): Boolean = call { sftp.stat(path).type == FileMode.Type.DIRECTORY }

    fun exists(path: String): Boolean = try {
        sftp.lstat(path)
        true
    } catch (e: SFTPException) {
        if (e.statusCode == Response.StatusCode.NO_SUCH_FILE) false else throw translate(e)
    } catch (e: IOException) {
        throw translate(e)
    }

    fun mkdir(path: String) = call { sftp.mkdir(path) }

    fun rename(from: String, to: String) = call { sftp.rename(from, to) }

    /**
     * Hapus file, symlink (link-nya saja), atau folder BESERTA isinya.
     * Tidak pernah masuk ke folder lewat symlink — yang dihapus link-nya,
     * bukan isi folder tujuannya.
     */
    fun delete(entry: RemoteEntry) = call { deleteRecursive(entry.path, entry.kind) }

    private fun deleteRecursive(path: String, kind: RemoteKind) {
        if (kind != RemoteKind.Directory) {
            sftp.rm(path)
            return
        }
        for (child in list(path)) deleteRecursive(child.path, child.kind)
        sftp.rmdir(path)
    }

    /** Salin file server -> [output]. [onProgress] menerima total byte yang sudah tersalin. */
    fun download(path: String, output: OutputStream, onProgress: (Long) -> Unit, isCancelled: () -> Boolean) = call {
        sftp.open(path, EnumSet.of(OpenMode.READ)).use { file ->
            file.ReadAheadRemoteFileInputStream(READ_AHEAD).use { input -> copy(input, output, onProgress, isCancelled) }
        }
    }

    /** Salin [input] -> file server (dibuat baru / ditimpa). */
    fun upload(input: InputStream, path: String, onProgress: (Long) -> Unit, isCancelled: () -> Boolean) = call {
        sftp.open(path, EnumSet.of(OpenMode.WRITE, OpenMode.CREAT, OpenMode.TRUNC)).use { file ->
            file.RemoteFileOutputStream(0, UNCONFIRMED_WRITES).use { output -> copy(input, output, onProgress, isCancelled) }
        }
    }

    fun close() {
        runCatching { sftp.close() }
        runCatching { client.disconnect() }
    }

    private fun copy(input: InputStream, output: OutputStream, onProgress: (Long) -> Unit, isCancelled: () -> Boolean) {
        val buffer = ByteArray(BUFFER_SIZE)
        var total = 0L
        while (true) {
            if (isCancelled()) throw TransferCancelled()
            val read = input.read(buffer)
            if (read < 0) break
            output.write(buffer, 0, read)
            total += read
            onProgress(total)
        }
        output.flush()
    }

    private fun <T> call(block: () -> T): T = try {
        block()
    } catch (e: TransferCancelled) {
        throw e
    } catch (e: SftpError) {
        throw e
    } catch (e: IOException) {
        throw translate(e)
    }

    private fun translate(e: IOException): SftpError {
        if (e is SFTPException) {
            val message = when (e.statusCode) {
                Response.StatusCode.NO_SUCH_FILE -> "File/folder tidak ada di server."
                Response.StatusCode.PERMISSION_DENIED -> "Izin ditolak server."
                Response.StatusCode.FILE_ALREADY_EXISTS -> "Nama itu sudah dipakai."
                Response.StatusCode.NO_SPACE_ON_FILESYSTEM, Response.StatusCode.QUOTA_EXCEEDED -> "Ruang di server penuh."
                Response.StatusCode.DIR_NOT_EMPTY -> "Folder tidak kosong."
                else -> "Server menolak: ${e.message ?: e.statusCode}"
            }
            return SftpError(message, e)
        }
        return SftpError(if (isConnected) "Gagal: ${e.message ?: e.javaClass.simpleName}" else "Koneksi SFTP terputus.", e)
    }

    private companion object {
        const val BUFFER_SIZE = 32 * 1024
        const val READ_AHEAD = 16
        const val UNCONFIRMED_WRITES = 16
    }
}
