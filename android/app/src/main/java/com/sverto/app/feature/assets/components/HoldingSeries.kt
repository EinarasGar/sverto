@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.assets.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ShowChart
import androidx.compose.material.icons.automirrored.outlined.ShowChart
import androidx.compose.material.icons.filled.AccountBalanceWallet
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Layers
import androidx.compose.material.icons.outlined.AccountBalanceWallet
import androidx.compose.material.icons.outlined.Layers
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import java.text.NumberFormat
import kotlin.math.roundToInt

enum class HoldingSeries(
    val label: String,
    val caption: String,
    val icon: ImageVector,
    val selectedIcon: ImageVector,
) {
    PRICE("Price", "Price per unit", Icons.AutoMirrored.Outlined.ShowChart, Icons.AutoMirrored.Filled.ShowChart),
    VALUE("Value", "Value of your holdings", Icons.Outlined.AccountBalanceWallet, Icons.Filled.AccountBalanceWallet),
    UNITS("Units", "Units you hold", Icons.Outlined.Layers, Icons.Filled.Layers),
}

@Composable
fun HoldingChartHeader(
    series: HoldingSeries,
    held: Boolean,
    onSeriesSelect: (HoldingSeries) -> Unit,
    currencyChip: @Composable () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            if (held) HoldingSeriesSwitch(selected = series, onSelect = onSeriesSelect)
            Spacer(Modifier.weight(1f))
            AnimatedVisibility(
                visible = series != HoldingSeries.UNITS,
                enter = fadeIn(MaterialTheme.motionScheme.fastEffectsSpec()),
                exit = fadeOut(MaterialTheme.motionScheme.fastEffectsSpec()),
            ) {
                currencyChip()
            }
        }
        if (held) {
            Text(
                text = series.caption,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

// The label is drawn unclipped and stays hidden until the button has opened this far, so its edge never cuts a letter.
private const val LABEL_FADE_START = 0.7f

@Composable
private fun HoldingSeriesSwitch(
    selected: HoldingSeries,
    onSelect: (HoldingSeries) -> Unit,
) {
    val colors =
        ToggleButtonDefaults.toggleButtonColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)
    val labelSpec = MaterialTheme.motionScheme.defaultSpatialSpec<Float>()
    val haptics = LocalHapticFeedback.current
    Row(horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween)) {
        HoldingSeries.entries.forEachIndexed { index, series ->
            val checked = series == selected
            ToggleButton(
                checked = checked,
                onCheckedChange = {
                    if (!checked) haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
                    onSelect(series)
                },
                shapes =
                    when (index) {
                        0 -> ButtonGroupDefaults.connectedLeadingButtonShapes()
                        HoldingSeries.entries.lastIndex -> ButtonGroupDefaults.connectedTrailingButtonShapes()
                        else -> ButtonGroupDefaults.connectedMiddleButtonShapes()
                    },
                colors = colors,
                modifier = Modifier.semantics { role = Role.RadioButton },
            ) {
                Icon(
                    imageVector = if (checked) series.selectedIcon else series.icon,
                    contentDescription = series.label,
                    modifier = Modifier.size(ButtonDefaults.IconSize),
                )
                val labelProgress by animateFloatAsState(if (checked) 1f else 0f, labelSpec, label = "label")
                Text(
                    text = series.label,
                    maxLines = 1,
                    softWrap = false,
                    modifier =
                        Modifier
                            .layout { measurable, _ ->
                                val label = measurable.measure(Constraints())
                                val width = (label.width * labelProgress).roundToInt().coerceAtLeast(0)
                                layout(width, label.height) { label.placeRelative(0, 0) }
                            }.graphicsLayer {
                                alpha = ((labelProgress - LABEL_FADE_START) / (1f - LABEL_FADE_START)).coerceIn(0f, 1f)
                            }.padding(start = ToggleButtonDefaults.IconSpacing)
                            .clearAndSetSemantics {},
                )
            }
        }
    }
}

@Composable
fun CurrencyChip(
    ticker: String,
    onClick: (() -> Unit)?,
) {
    if (onClick == null) {
        Surface(shape = ButtonDefaults.shape, color = MaterialTheme.colorScheme.secondaryContainer) {
            Text(
                text = ticker,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            )
        }
        return
    }
    FilledTonalButton(
        onClick = onClick,
        shapes = ButtonDefaults.shapes(),
        contentPadding = PaddingValues(start = 16.dp, top = 8.dp, end = 12.dp, bottom = 8.dp),
    ) {
        Text(ticker, maxLines = 1)
        Spacer(Modifier.size(ButtonDefaults.IconSpacing))
        Icon(
            Icons.Filled.ArrowDropDown,
            contentDescription = "Change currency",
            modifier = Modifier.size(ButtonDefaults.IconSize),
        )
    }
}

fun formatQuantity(
    amount: Double,
    signed: Boolean,
): String {
    val formatted = NumberFormat.getNumberInstance().apply { maximumFractionDigits = 4 }.format(amount)
    return if (signed && amount > 0) "+$formatted" else formatted
}
