package com.sverto.app.feature.portfolio

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ShowChart
import androidx.compose.material.icons.outlined.AccountBalance
import androidx.compose.material.icons.outlined.CurrencyBitcoin
import androidx.compose.material.icons.outlined.DirectionsCar
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Layers
import androidx.compose.material.icons.outlined.Payments
import androidx.compose.material.icons.outlined.PieChart
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sverto.app.core.Money
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.accounts.components.accountTypeIcon
import com.sverto.app.feature.accounts.components.accountTypeLabel
import uniffi.sverto_core.AccountCurrencyBalance
import uniffi.sverto_core.HoldingItem
import uniffi.sverto_core.LiabilityItem
import uniffi.sverto_core.PropertyKind
import java.text.NumberFormat
import java.util.Locale
import kotlin.math.abs

@Composable
fun HoldingsList(
    holdings: List<HoldingItem>,
    liabilities: List<LiabilityItem>,
    holdingsTotal: Double,
    liabilitiesTotal: Double,
    baseTicker: String,
    onHoldingClick: (HoldingItem) -> Unit,
    onLiabilityClick: (LiabilityItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (holdings.isEmpty() && liabilities.isEmpty()) return

    Column(modifier, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        if (holdings.isNotEmpty()) {
            Column {
                SectionHeader(title = "Holdings", total = Money.format(holdingsTotal, baseTicker))
                Surface(
                    shape = RoundedCornerShape(20.dp),
                    color = MaterialTheme.colorScheme.surfaceBright,
                ) {
                    Column {
                        holdings.forEachIndexed { index, holding ->
                            HoldingRow(holding, baseTicker, onClick = { onHoldingClick(holding) })
                            if (index < holdings.lastIndex) {
                                RowDivider()
                            }
                        }
                    }
                }
            }
        }

        if (liabilities.isNotEmpty()) {
            Column {
                SectionHeader(title = "Liabilities", total = Money.format(liabilitiesTotal, baseTicker))
                Surface(
                    shape = RoundedCornerShape(20.dp),
                    color = MaterialTheme.colorScheme.surfaceBright,
                ) {
                    Column {
                        liabilities.forEachIndexed { index, liability ->
                            LiabilityRow(liability, baseTicker, onClick = { onLiabilityClick(liability) })
                            if (index < liabilities.lastIndex) {
                                RowDivider()
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SectionHeader(
    title: String,
    total: String,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(bottom = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = total,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
        )
    }
}

@Composable
private fun HoldingRow(
    holding: HoldingItem,
    baseTicker: String,
    onClick: () -> Unit,
) {
    val property = holding.property
    val isProperty = isPropertyAssetType(holding.assetTypeId)

    PortfolioRow(
        icon = holdingIcon(holding.assetTypeId),
        iconTint = holdingIconTint(holding.assetTypeId),
        title = holding.assetName,
        subtitle =
            when {
                property != null -> {
                    val kind =
                        when (property.kind) {
                            PropertyKind.REAL_ESTATE -> "Property"
                            PropertyKind.VEHICLE -> "Vehicle"
                        }
                    if (property.loanBalance != 0.0) {
                        "$kind · Value ${Money.format(property.assetValue, baseTicker)}"
                    } else {
                        kind
                    }
                }
                isProperty -> null
                else -> holding.ticker
            },
        value = Money.format(holding.value, baseTicker),
        detail =
            when {
                property != null && property.loanBalance != 0.0 ->
                    "Loan ${Money.format(abs(property.loanBalance), baseTicker)}"
                property != null || isProperty -> null
                else -> formatUnits(holding.units) + " units"
            },
        onClick = onClick,
    )
}

@Composable
private fun LiabilityRow(
    liability: LiabilityItem,
    baseTicker: String,
    onClick: () -> Unit,
) {
    PortfolioRow(
        icon = accountTypeIcon(liability.accountTypeId),
        iconTint = MaterialTheme.colorScheme.error,
        title = liability.accountName,
        subtitle = accountTypeLabel(liability.accountTypeId),
        value = Money.format(liability.value, baseTicker),
        detail = null,
        onClick = onClick,
    )
}

@Composable
internal fun CurrencyBalanceRow(
    balance: AccountCurrencyBalance,
    baseTicker: String,
    onClick: () -> Unit,
) {
    val baseValue = balance.value?.takeIf { balance.ticker != baseTicker }
    PortfolioRow(
        icon = holdingIcon(ASSET_TYPE_CURRENCIES),
        iconTint = holdingIconTint(ASSET_TYPE_CURRENCIES),
        title = balance.ticker,
        subtitle = balance.name,
        value = Money.format(balance.units, balance.ticker),
        detail = baseValue?.let { Money.format(it, baseTicker) },
        onClick = onClick,
    )
}

@Composable
private fun PortfolioRow(
    icon: ImageVector,
    iconTint: Color,
    title: String,
    subtitle: String?,
    value: String,
    detail: String?,
    onClick: () -> Unit,
) {
    ListItem(
        modifier = Modifier.clickable(onClick = onClick),
        colors =
            ListItemDefaults.colors(
                containerColor = MaterialTheme.colorScheme.surfaceBright,
            ),
        leadingContent = {
            Box(
                modifier =
                    Modifier
                        .size(40.dp)
                        .background(
                            color = iconTint.copy(alpha = 0.12f),
                            shape = RoundedCornerShape(12.dp),
                        ),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = title,
                    tint = iconTint,
                    modifier = Modifier.size(20.dp),
                )
            }
        },
        headlineContent = {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        },
        supportingContent =
            subtitle?.let {
                {
                    Text(
                        text = it,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            },
        trailingContent = {
            Column(horizontalAlignment = Alignment.End) {
                Text(
                    text = value,
                    style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.Medium),
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 1,
                )
                if (detail != null) {
                    Text(
                        text = detail,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
        },
    )
}

private const val ASSET_TYPE_CURRENCIES = 1
private const val ASSET_TYPE_STOCKS = 2
private const val ASSET_TYPE_BONDS = 3
private const val ASSET_TYPE_MUTUAL_FUNDS = 4
private const val ASSET_TYPE_ETFS = 5
private const val ASSET_TYPE_ETCS = 6
private const val ASSET_TYPE_CRYPTO = 7
private const val ASSET_TYPE_REAL_ESTATE = 8
private const val ASSET_TYPE_VEHICLES = 9

// A property is one unit with a generated ticker, so its rows show the name and no units.
internal fun isPropertyAssetType(assetTypeId: Int) = assetTypeId == ASSET_TYPE_REAL_ESTATE || assetTypeId == ASSET_TYPE_VEHICLES

internal fun isCurrencyAssetType(assetTypeId: Int) = assetTypeId == ASSET_TYPE_CURRENCIES

private fun holdingIcon(assetTypeId: Int): ImageVector =
    when (assetTypeId) {
        ASSET_TYPE_CURRENCIES -> Icons.Outlined.Payments
        ASSET_TYPE_STOCKS -> Icons.AutoMirrored.Outlined.ShowChart
        ASSET_TYPE_BONDS -> Icons.Outlined.AccountBalance
        ASSET_TYPE_MUTUAL_FUNDS, ASSET_TYPE_ETFS, ASSET_TYPE_ETCS -> Icons.Outlined.PieChart
        ASSET_TYPE_CRYPTO -> Icons.Outlined.CurrencyBitcoin
        ASSET_TYPE_REAL_ESTATE -> Icons.Outlined.Home
        ASSET_TYPE_VEHICLES -> Icons.Outlined.DirectionsCar
        else -> Icons.Outlined.Layers
    }

@Composable
private fun holdingIconTint(assetTypeId: Int) =
    when (assetTypeId) {
        ASSET_TYPE_CURRENCIES -> MaterialTheme.colorScheme.primary
        ASSET_TYPE_CRYPTO -> MaterialTheme.colorScheme.secondary
        ASSET_TYPE_REAL_ESTATE, ASSET_TYPE_VEHICLES -> MaterialTheme.colorScheme.secondary
        else -> MaterialTheme.colorScheme.tertiary
    }

private fun formatUnits(units: Double): String {
    val fmt = NumberFormat.getNumberInstance(Locale.getDefault())
    fmt.maximumFractionDigits = 4
    return fmt.format(units)
}
