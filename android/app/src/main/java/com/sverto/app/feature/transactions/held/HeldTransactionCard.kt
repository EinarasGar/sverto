package com.sverto.app.feature.transactions.held

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.feature.connectors.CONNECTORS
import com.sverto.app.feature.imports.review.importDate
import uniffi.sverto_core.HeldTransaction
import uniffi.sverto_core.HeldTransactionDecision

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HeldTransactionCard(
    held: HeldTransaction,
    deciding: Boolean,
    enabled: Boolean,
    error: String?,
    onDecide: (HeldTransactionDecision) -> Unit,
    modifier: Modifier = Modifier,
) {
    var candidateIndex by rememberSaveable(held.id) { mutableIntStateOf(0) }
    val index = if (candidateIndex < held.candidates.size) candidateIndex else 0
    val candidate = held.candidates.getOrNull(index)
    val connector = CONNECTORS.firstOrNull { it.kind == held.providerKind }
    val subject = "${held.description}, ${held.amountDisplay}"

    ElevatedCard(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            ComparedTransaction(
                label = listOf(connector?.name ?: held.providerKind, held.accountName).filter { it.isNotBlank() }.joinToString(" · "),
                icon = connector?.icon ?: "landmark",
                description = held.description,
                detail = importDate(held.date),
                amount = held.amountDisplay,
            )

            Spacer(Modifier.height(12.dp))

            if (candidate == null) {
                Text(
                    text = "The transaction it looked like was deleted or already matched another payment from this bank account.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                ComparedTransaction(
                    label =
                        if (held.candidates.size > 1) {
                            "Already in Sverto · ${index + 1} of ${held.candidates.size}"
                        } else {
                            "Already in Sverto"
                        },
                    icon = candidate.categoryIcon ?: "receipt",
                    description = candidate.description ?: "No description",
                    detail = listOfNotNull(importDate(candidate.date), candidate.categoryName).joinToString(" · "),
                    amount = candidate.amountDisplay,
                )
                if (held.candidates.size > 1) {
                    TextButton(
                        onClick = { candidateIndex = (index + 1) % held.candidates.size },
                        enabled = enabled,
                    ) { Text("Compare with another (${held.candidates.size - 1})") }
                }
            }

            if (error != null) {
                Spacer(Modifier.height(8.dp))
                Text(
                    text = error,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            if (deciding) {
                Spacer(Modifier.height(10.dp))
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }

            Spacer(Modifier.height(10.dp))

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (candidate != null) {
                    Button(
                        onClick = { onDecide(HeldTransactionDecision.Same(candidate.id)) },
                        enabled = enabled,
                        shapes = ButtonDefaults.shapes(),
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .semantics { contentDescription = "Same transaction: $subject" },
                    ) { Text("Same transaction") }
                    OutlinedButton(
                        onClick = { onDecide(HeldTransactionDecision.Different) },
                        enabled = enabled,
                        shapes = ButtonDefaults.shapes(),
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .semantics { contentDescription = "Different, add it: $subject" },
                    ) { Text("Different — add it") }
                } else {
                    Button(
                        onClick = { onDecide(HeldTransactionDecision.Different) },
                        enabled = enabled,
                        shapes = ButtonDefaults.shapes(),
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .semantics { contentDescription = "Add it: $subject" },
                    ) { Text("Add it") }
                }
            }
        }
    }
}

@Composable
private fun ComparedTransaction(
    label: String,
    icon: String,
    description: String,
    detail: String,
    amount: String,
) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(6.dp))
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHighest,
        ) {
            Row(
                modifier = Modifier.padding(12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                LucideIcon(
                    name = icon,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = description,
                        style = MaterialTheme.typography.bodyMedium,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = detail,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(Modifier.width(12.dp))
                Text(
                    text = amount,
                    style = MaterialTheme.typography.bodyLarge,
                    maxLines = 1,
                )
            }
        }
    }
}
