package com.sverto.app.feature.imports.review

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon

enum class ImportTone { Ghost, Brand, Attention, Positive, Negative }

@Composable
fun ImportTone.containerColor(): Color =
    when (this) {
        ImportTone.Ghost -> MaterialTheme.colorScheme.surfaceContainerHighest
        ImportTone.Brand -> MaterialTheme.colorScheme.primaryContainer
        ImportTone.Attention -> MaterialTheme.colorScheme.tertiaryContainer
        ImportTone.Positive -> MaterialTheme.colorScheme.secondaryContainer
        ImportTone.Negative -> MaterialTheme.colorScheme.errorContainer
    }

@Composable
fun ImportTone.contentColor(): Color =
    when (this) {
        ImportTone.Ghost -> MaterialTheme.colorScheme.onSurfaceVariant
        ImportTone.Brand -> MaterialTheme.colorScheme.onPrimaryContainer
        ImportTone.Attention -> MaterialTheme.colorScheme.onTertiaryContainer
        ImportTone.Positive -> MaterialTheme.colorScheme.onSecondaryContainer
        ImportTone.Negative -> MaterialTheme.colorScheme.onErrorContainer
    }

@Composable
fun ToneChip(
    label: String,
    tone: ImportTone,
    modifier: Modifier = Modifier,
    icon: String? = null,
) {
    Row(
        modifier =
            modifier
                .clip(RoundedCornerShape(50))
                .background(tone.containerColor())
                .padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            LucideIcon(name = icon, tint = tone.contentColor(), modifier = Modifier.size(14.dp))
        } else {
            Box(
                modifier =
                    Modifier
                        .size(6.dp)
                        .clip(CircleShape)
                        .background(tone.contentColor()),
            )
        }
        Spacer(Modifier.width(6.dp))
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = tone.contentColor(),
            maxLines = 1,
        )
    }
}
