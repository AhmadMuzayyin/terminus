package org.terminus.mobile.data

import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostGroup

/** Satu baris grup di daftar Hosts. */
data class GroupEntry(val group: HostGroup, val hostCount: Int)

/** Isi layar Hosts di satu posisi drill-down. */
data class HostListing(val groups: List<GroupEntry>, val hosts: List<Host>)

/**
 * Isi layar Hosts (mobile/DESIGN.md bagian 7): di akar = semua grup + host
 * tanpa grup; di dalam grup = host grup itu saja. Grup satu tingkat, sama
 * dengan desktop. Host yang `groupId`-nya menunjuk grup yang sudah tidak
 * ada ikut tampil di akar — kalau tidak, host itu tidak terlihat di mana pun.
 */
fun hostListing(groups: List<HostGroup>, hosts: List<Host>, openGroupId: String?): HostListing {
    val sortedHosts = hosts.sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.label })
    if (openGroupId != null) {
        return HostListing(emptyList(), sortedHosts.filter { it.groupId == openGroupId })
    }
    val groupIds = groups.mapTo(HashSet()) { it.id }
    val counts = hosts.groupingBy { it.groupId }.eachCount()
    return HostListing(
        groups = groups
            .sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })
            .map { GroupEntry(it, counts[it.id] ?: 0) },
        hosts = sortedHosts.filter { it.groupId == null || it.groupId !in groupIds },
    )
}

/**
 * Pencarian: SEMUA host (lintas grup) yang label, host, username, atau
 * nama grupnya mengandung setiap kata di [query] (tidak peka huruf besar).
 */
fun searchHosts(groups: List<HostGroup>, hosts: List<Host>, query: String): List<Host> {
    val words = query.trim().lowercase().split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (words.isEmpty()) return emptyList()
    val groupNames = groups.associate { it.id to it.name }
    return hosts
        .filter { host ->
            val haystack = listOfNotNull(host.label, host.host, host.username, groupNames[host.groupId])
                .joinToString(" ")
                .lowercase()
            words.all { it in haystack }
        }
        .sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.label })
}

/** Alamat ringkas buat subjudul: `user@host` + `:port` kalau bukan 22. */
fun Host.address(): String {
    val target = if (username.isNotEmpty()) "$username@$host" else host
    return if (port == 22) target else "$target:$port"
}
