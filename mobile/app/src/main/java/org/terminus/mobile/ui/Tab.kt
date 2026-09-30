package org.terminus.mobile.ui

import androidx.annotation.DrawableRes
import androidx.annotation.StringRes
import org.terminus.mobile.R

/** Tab bottom navigation — urutan = urutan tampil (mobile/DESIGN.md bagian 7). */
enum class Tab(@StringRes val label: Int, @DrawableRes val icon: Int) {
    Hosts(R.string.tab_hosts, R.drawable.ic_server),
    Sftp(R.string.tab_sftp, R.drawable.ic_folder),
    Identities(R.string.tab_identities, R.drawable.ic_key),
    Account(R.string.tab_account, R.drawable.ic_user),
}
