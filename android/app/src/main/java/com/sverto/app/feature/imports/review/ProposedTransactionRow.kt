package com.sverto.app.feature.imports.review

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.feature.imports.verbLabel
import com.sverto.app.feature.imports.verbTone
import com.sverto.app.feature.transactions.signedSecondary
import com.sverto.app.feature.transactions.transactionTypeIcon
import uniffi.sverto_core.ImportRowVerb
import uniffi.sverto_core.ImportStagedRow

fun ImportStagedRow.supportingLine(withDate: Boolean = false): String =
    listOfNotNull(
        if (withDate) importDate(date) else null,
        categoryName ?: typeLabel.takeIf { it != description },
        accountName,
    ).filter { it.isNotBlank() }
        .joinToString(" · ")

@Composable
fun ProposedTransactionRow(
    row: ImportStagedRow,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    showVerb: Boolean = true,
    showDate: Boolean = false,
) {
    val skipped = row.verb == ImportRowVerb.SKIP
    val textColor = if (skipped) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface
    val chip = (showVerb && row.verb != ImportRowVerb.ADD) || row.verbLocked
    ListItem(
        modifier = modifier.clickable(onClick = onClick),
        colors = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.surfaceBright),
        leadingContent = { ProposedRowGlyph(row, Modifier.size(24.dp)) },
        headlineContent = {
            Text(
                text = row.description,
                style = MaterialTheme.typography.bodyLarge,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        },
        supportingContent = {
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = row.supportingLine(showDate),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (chip) VerbChip(verb = row.verb, locked = row.verbLocked)
            }
        },
        trailingContent = {
            Column(horizontalAlignment = Alignment.End) {
                Text(text = row.amountDisplay, style = MaterialTheme.typography.bodyLarge, color = textColor, maxLines = 1)
                row.secondaryAmountDisplay?.let {
                    Text(
                        text = signedSecondary(it),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
        },
    )
}

@Composable
fun ProposedRowGlyph(
    row: ImportStagedRow,
    modifier: Modifier = Modifier,
    tint: Color = MaterialTheme.colorScheme.onSurfaceVariant,
) {
    val icon = row.categoryIcon
    if (icon != null) {
        LucideIcon(name = icon, tint = tint, modifier = modifier)
    } else {
        Icon(
            imageVector = transactionTypeIcon(row.typeKey),
            contentDescription = row.typeLabel,
            tint = tint,
            modifier = modifier,
        )
    }
}

@Composable
fun VerbChip(
    verb: ImportRowVerb,
    locked: Boolean,
    modifier: Modifier = Modifier,
    iconSize: Dp = 12.dp,
) {
    val tone = verbTone(verb)
    Row(
        modifier =
            modifier
                .clip(RoundedCornerShape(8.dp))
                .background(tone.containerColor())
                .semantics(mergeDescendants = true) {
                    contentDescription = if (locked) "${verbLabel(verb)}, your choice" else verbLabel(verb)
                }.padding(horizontal = 8.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (locked) {
            LucideIcon(name = "lock", tint = tone.contentColor(), modifier = Modifier.size(iconSize))
        }
        Text(text = verbLabel(verb), style = MaterialTheme.typography.labelSmall, color = tone.contentColor())
    }
}
