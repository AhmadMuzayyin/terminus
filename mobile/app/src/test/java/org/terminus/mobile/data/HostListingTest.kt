package org.terminus.mobile.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.terminus.mobile.api.Host
import org.terminus.mobile.api.HostGroup

class HostListingTest {
    private val prod = HostGroup("g-prod", "Production")
    private val lab = HostGroup("g-lab", "lab")
    private val groups = listOf(prod, lab)

    private fun host(id: String, label: String, groupId: String? = null, host: String = "$id.lan", username: String = "") =
        Host(id = id, label = label, host = host, username = username, groupId = groupId)

    private val hosts = listOf(
        host("web", "web-01", prod.id, username = "deploy"),
        host("db", "DB-01", prod.id),
        host("pi", "raspberry", lab.id, host = "192.168.1.50"),
        host("nas", "nas"),
        host("orphan", "yatim", groupId = "grup-sudah-dihapus"),
    )

    @Test fun `akar - grup urut nama beserta jumlah host, lalu host tanpa grup termasuk yatim`() {
        val listing = hostListing(groups, hosts, openGroupId = null)
        assertEquals(listOf("lab" to 1, "Production" to 2), listing.groups.map { it.group.name to it.hostCount })
        assertEquals(listOf("nas", "yatim"), listing.hosts.map { it.label })
    }

    @Test fun `di dalam grup - cuma host grup itu, urut label tanpa peka huruf besar`() {
        val listing = hostListing(groups, hosts, openGroupId = prod.id)
        assertTrue(listing.groups.isEmpty())
        assertEquals(listOf("DB-01", "web-01"), listing.hosts.map { it.label })
    }

    @Test fun `cari lintas grup lewat label, IP, username, dan nama grup`() {
        assertEquals(listOf("raspberry"), searchHosts(groups, hosts, "192.168").map { it.label })
        assertEquals(listOf("web-01"), searchHosts(groups, hosts, "DEPLOY").map { it.label })
        assertEquals(listOf("DB-01", "web-01"), searchHosts(groups, hosts, "production").map { it.label })
        // Semua kata harus cocok (boleh di field berbeda).
        assertEquals(listOf("web-01"), searchHosts(groups, hosts, "prod web").map { it.label })
        assertTrue(searchHosts(groups, hosts, "   ").isEmpty())
    }

    @Test fun `alamat ringkas - username dan port non-22 opsional`() {
        assertEquals("h.lan", Host("1", "l", "h.lan").address())
        assertEquals("root@h.lan", Host("1", "l", "h.lan", username = "root").address())
        assertEquals("root@h.lan:2222", Host("1", "l", "h.lan", port = 2222, username = "root").address())
    }
}
