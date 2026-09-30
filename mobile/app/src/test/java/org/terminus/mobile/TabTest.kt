package org.terminus.mobile

import org.junit.Assert.assertEquals
import org.terminus.mobile.ui.Tab
import org.junit.Test

class TabTest {
    /** Urutan tab = kontrak desain (mobile/DESIGN.md bagian 7). */
    @Test
    fun urutan_tab_sesuai_desain() {
        assertEquals(listOf(Tab.Hosts, Tab.Sftp, Tab.Identities, Tab.Account), Tab.entries.toList())
    }
}
