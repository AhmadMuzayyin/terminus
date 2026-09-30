package org.terminus.mobile.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import org.terminus.mobile.api.Account
import org.terminus.mobile.ui.account.AccountScreen
import org.terminus.mobile.ui.theme.TerminusTheme

// Layar utama SETELAH login: bottom navigation. Tab dipilih lewat state
// biasa (bukan navigation-compose) — layar penuh Terminal (Milestone 4)
// yang nanti menentukan perlu tidaknya navigasi bertingkat.
@Composable
fun TerminusApp(account: Account, serverUrl: String, onLogout: () -> Unit) {
    var current by rememberSaveable { mutableStateOf(Tab.Hosts) }

    Scaffold(
        bottomBar = {
            NavigationBar {
                Tab.entries.forEach { tab ->
                    NavigationBarItem(
                        selected = tab == current,
                        onClick = { current = tab },
                        icon = { Icon(painterResource(tab.icon), contentDescription = null) },
                        label = { Text(stringResource(tab.label)) },
                    )
                }
            }
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding)) {
            when (current) {
                Tab.Account -> AccountScreen(account, serverUrl, onLogout)
                else -> PlaceholderScreen(title = stringResource(current.label))
            }
        }
    }
}

@Composable
private fun PlaceholderScreen(title: String) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = androidx.compose.foundation.layout.Arrangement.Center,
    ) {
        Text(title, style = MaterialTheme.typography.headlineMedium)
        Text(
            "Belum diimplementasi — lihat mobile/DESIGN.md bagian 9.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(top = 8.dp),
        )
    }
}

@Preview
@Composable
private fun TerminusAppPreview() {
    TerminusTheme {
        TerminusApp(Account(id = "1", email = "admin@contoh.com", fullName = "Admin"), "https://vault.contoh.com", onLogout = {})
    }
}
