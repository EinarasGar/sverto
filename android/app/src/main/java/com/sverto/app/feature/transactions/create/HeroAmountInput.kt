package com.sverto.app.feature.transactions.create

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material3.AssistChip
import androidx.compose.material3.AssistChipDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HeroAmountInput(
    value: String,
    onValueChange: (String) -> Unit,
    sign: AmountSign,
    modifier: Modifier = Modifier,
    label: String = "Amount",
) {
    val focusRequester = remember { FocusRequester() }

    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    val isNegative =
        when (sign) {
            AmountSign.POSITIVE -> false
            AmountSign.NEGATIVE -> true
            AmountSign.ANY -> value.startsWith("-")
        }
    val magnitude = value.removePrefix("-")
    val prefix = if (isNegative) "−" else "+"
    val prefixColor = if (isNegative) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary

    val numberStyle =
        MaterialTheme.typography.displayMedium.copy(
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurface,
        )

    val motionScheme = MaterialTheme.motionScheme

    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceBright,
        modifier =
            modifier
                .fillMaxWidth()
                .semantics { contentDescription = "Amount input" },
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(Modifier.height(4.dp))
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.Center,
                modifier = Modifier.animateContentSize(motionScheme.fastSpatialSpec()),
            ) {
                Text(
                    text = prefix,
                    style = numberStyle.copy(color = prefixColor),
                )
                Spacer(Modifier.width(4.dp))
                Box(contentAlignment = Alignment.Center) {
                    if (magnitude.isEmpty()) {
                        Text(
                            text = "0",
                            style =
                                numberStyle.copy(
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                ),
                        )
                    }
                    BasicTextField(
                        value = magnitude,
                        onValueChange = { new ->
                            val filtered = filterAmountInput(new)
                            onValueChange(if (sign == AmountSign.ANY && isNegative) "-$filtered" else filtered)
                        },
                        singleLine = true,
                        textStyle = LocalTextStyle.current.merge(numberStyle),
                        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                        keyboardOptions =
                            KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        modifier = Modifier.focusRequester(focusRequester),
                    )
                }
            }
            if (sign == AmountSign.ANY) {
                Spacer(Modifier.height(8.dp))
                SignToggleChip(
                    isNegative = isNegative,
                    onToggle = { onValueChange(if (isNegative) magnitude else "-$magnitude") },
                )
            }
        }
    }
}

@Composable
private fun SignToggleChip(
    isNegative: Boolean,
    onToggle: () -> Unit,
) {
    val color = if (isNegative) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary
    AssistChip(
        onClick = onToggle,
        label = { Text(if (isNegative) "Expense" else "Income") },
        leadingIcon = {
            Icon(
                imageVector = if (isNegative) Icons.Default.Remove else Icons.Default.Add,
                contentDescription = null,
                modifier = Modifier.size(AssistChipDefaults.IconSize),
            )
        },
        shape = CircleShape,
        colors =
            AssistChipDefaults.assistChipColors(
                labelColor = color,
                leadingIconContentColor = color,
            ),
        border = BorderStroke(1.dp, color.copy(alpha = 0.5f)),
        modifier =
            Modifier.semantics {
                contentDescription = if (isNegative) "Expense, tap for income" else "Income, tap for expense"
            },
    )
}
