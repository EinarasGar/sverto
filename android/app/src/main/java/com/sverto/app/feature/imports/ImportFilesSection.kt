@file:OptIn(ExperimentalMaterial3ExpressiveApi::class)

package com.sverto.app.feature.imports

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sverto.app.core.icons.LucideIcon
import com.sverto.app.core.ui.RowDivider
import com.sverto.app.feature.connectors.CONNECTORS
import com.sverto.app.feature.connectors.humanizeAccountName
import com.sverto.app.feature.imports.review.SectionCount
import com.sverto.app.feature.imports.review.SectionHeader
import uniffi.sverto_core.ImportDateOrder
import uniffi.sverto_core.ImportFile
import uniffi.sverto_core.ImportFileStatus
import uniffi.sverto_core.ImportSource
import uniffi.sverto_core.ImportSourceKind
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

@Composable
fun ImportFilesSection(
    sources: List<ImportSource>,
    uploads: List<LocalUpload>,
    acceptsFiles: Boolean,
    enabled: Boolean,
    collapsible: Boolean,
    expanded: Boolean,
    onToggle: () -> Unit,
    onAddFiles: () -> Unit,
    onRetryFile: (String) -> Unit,
    onRemoveFile: (String) -> Unit,
    onDateOrder: (String, ImportDateOrder) -> Unit,
    onRetryUpload: (String) -> Unit,
    onDismissUpload: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var removing by remember { mutableStateOf<ImportFile?>(null) }
    val rows: @Composable () -> Unit = {
        sources.forEachIndexed { index, source ->
            if (index > 0) RowDivider()
            when (val kind = source.kind) {
                is ImportSourceKind.File ->
                    FileRow(
                        file = kind.file,
                        recordedTransactions = source.recordedTransactions,
                        acceptsFiles = acceptsFiles,
                        enabled = enabled,
                        onRetry = { onRetryFile(kind.file.fileId) },
                        onRemove = { removing = kind.file },
                        onDateOrder = { onDateOrder(kind.file.fileId, it) },
                    )
                is ImportSourceKind.BankAccount -> BankAccountRow(kind, source.recordedTransactions)
            }
        }
        uploads.forEachIndexed { index, upload ->
            if (sources.isNotEmpty() || index > 0) RowDivider()
            UploadRow(upload, { onRetryUpload(upload.id) }, { onDismissUpload(upload.id) })
        }
    }
    val hasBankAccounts = sources.any { it.kind is ImportSourceKind.BankAccount }
    val hasFiles = uploads.isNotEmpty() || sources.any { it.kind is ImportSourceKind.File }
    Column(modifier = modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        if (collapsible) {
            CollapsedFiles(
                title = sourcesTitle(hasFiles, hasBankAccounts, filesTitle = "Source files"),
                icon = if (hasFiles) "files" else "landmark",
                count = sources.size + uploads.size,
                acceptsFiles = acceptsFiles,
                expanded = expanded,
                onToggle = onToggle,
                onAddFiles = onAddFiles,
                rows = rows,
            )
        } else {
            val empty = sources.isEmpty() && uploads.isEmpty()
            if (!empty) {
                SectionHeader(
                    title = sourcesTitle(hasFiles, hasBankAccounts, filesTitle = "Files"),
                    trailing = { SectionCount(sourcesCount(sources, uploads.size)) },
                )
                Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
                    Column { rows() }
                }
            }
            if (acceptsFiles) {
                AddFilesButton(prominent = empty, onClick = onAddFiles)
                if (empty) {
                    Text(
                        text = FILES_FOOTNOTE,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                }
            }
        }
        removing?.let { file ->
            AlertDialog(
                onDismissRequest = { removing = null },
                title = { Text("Remove this file?") },
                text = { Text("“${file.name}” and its transactions leave this import. Nothing in your ledger changes.") },
                confirmButton = {
                    TextButton(onClick = {
                        removing = null
                        onRemoveFile(file.fileId)
                    }) { Text("Remove") }
                },
                dismissButton = { TextButton(onClick = { removing = null }) { Text("Cancel") } },
            )
        }
    }
}

@Composable
private fun CollapsedFiles(
    title: String,
    icon: String,
    count: Int,
    acceptsFiles: Boolean,
    expanded: Boolean,
    onToggle: () -> Unit,
    onAddFiles: () -> Unit,
    rows: @Composable () -> Unit,
) {
    val rotation by animateFloatAsState(
        targetValue = if (expanded) 180f else 0f,
        animationSpec = MaterialTheme.motionScheme.defaultSpatialSpec(),
        label = "filesChevron",
    )
    Surface(shape = RoundedCornerShape(20.dp), color = MaterialTheme.colorScheme.surfaceBright) {
        Column {
            Row(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .clickable(
                            onClickLabel = "${if (expanded) "Hide" else "Show"} ${title.lowercase()}",
                            onClick = onToggle,
                        ).heightIn(min = 64.dp)
                        .padding(start = 20.dp, end = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                LucideIcon(name = icon, tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(20.dp))
                Spacer(Modifier.width(16.dp))
                Text(
                    text = "$title · $count",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(1f),
                )
                if (acceptsFiles) {
                    TextButton(onClick = onAddFiles, shapes = ButtonDefaults.shapes()) {
                        LucideIcon(name = "plus", tint = LocalContentColor.current, modifier = Modifier.size(ButtonDefaults.IconSize))
                        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                        Text("Add")
                    }
                }
                Icon(
                    imageVector = Icons.Filled.ExpandMore,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.rotate(rotation),
                )
            }
            AnimatedVisibility(visible = expanded) {
                Column {
                    RowDivider()
                    rows()
                }
            }
        }
    }
}

@Composable
private fun AddFilesButton(
    prominent: Boolean,
    onClick: () -> Unit,
) {
    val content: @Composable () -> Unit = {
        LucideIcon(name = "file-up", tint = LocalContentColor.current, modifier = Modifier.size(ButtonDefaults.IconSize))
        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
        Text("Add files")
    }
    val modifier =
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
    if (prominent) {
        Button(onClick = onClick, shapes = ButtonDefaults.shapes(), modifier = modifier) { content() }
    } else {
        OutlinedButton(onClick = onClick, shapes = ButtonDefaults.shapes(), modifier = modifier) { content() }
    }
}

@Composable
private fun FileRow(
    file: ImportFile,
    recordedTransactions: Long,
    acceptsFiles: Boolean,
    enabled: Boolean,
    onRetry: () -> Unit,
    onRemove: () -> Unit,
    onDateOrder: (ImportDateOrder) -> Unit,
) {
    val failed = file.status == ImportFileStatus.FAILED
    val reading = file.pausedUntil == null && file.status == ImportFileStatus.PROCESSING
    val dateOrder = file.dateOrder.takeIf { file.status == ImportFileStatus.PARSED }
    SourceRowLayout(
        icon = fileGlyph(file.name),
        name = file.name,
        status = fileStatusLine(file, recordedTransactions),
        statusColor = if (failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        note = dateOrder?.let(::dateOrderNote),
        loading = reading,
        hasActions = acceptsFiles,
        action =
            if (failed && acceptsFiles) {
                {
                    FilledTonalButton(
                        onClick = onRetry,
                        enabled = enabled,
                        shapes = ButtonDefaults.shapes(),
                        modifier =
                            Modifier
                                .padding(top = 8.dp)
                                .semantics { contentDescription = "Retry reading ${file.name}" },
                    ) {
                        Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(ButtonDefaults.IconSize))
                        Spacer(Modifier.width(ButtonDefaults.IconSpacing))
                        Text("Retry")
                    }
                }
            } else {
                null
            },
    ) {
        if (acceptsFiles) {
            FileMenu(
                name = file.name,
                enabled = enabled,
                dateOrder = dateOrder,
                onDateOrder = onDateOrder,
                onRemove = onRemove,
            )
        }
    }
}

@Composable
private fun BankAccountRow(
    account: ImportSourceKind.BankAccount,
    recordedTransactions: Long,
) {
    SourceRowLayout(
        icon = "landmark",
        name = bankAccountLabel(account),
        status =
            listOfNotNull(
                account.currency,
                account.accountNumberLast4?.let { "••$it" },
                providerName(account.providerKind),
            ).joinToString(" · "),
        statusColor = MaterialTheme.colorScheme.onSurfaceVariant,
        note = recordedTransactions.takeIf { it > 0 }?.let { "${countOf(it, "transaction")} recorded" },
        loading = false,
        hasActions = false,
        action = null,
    ) {}
}

@Composable
private fun UploadRow(
    upload: LocalUpload,
    onRetry: () -> Unit,
    onDismiss: () -> Unit,
) {
    SourceRowLayout(
        icon = fileGlyph(upload.name),
        name = upload.name,
        status = if (upload.failed) "Upload failed · ${upload.error ?: "Try again"}" else "Uploading…",
        statusColor = if (upload.failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        note = null,
        loading = !upload.failed,
        hasActions = upload.failed,
        action = null,
    ) {
        if (upload.failed) {
            Row {
                IconButton(onClick = onRetry) {
                    Icon(Icons.Filled.Refresh, contentDescription = "Retry upload")
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Filled.Close, contentDescription = "Remove upload")
                }
            }
        }
    }
}

@Composable
private fun SourceRowLayout(
    icon: String,
    name: String,
    status: String,
    statusColor: Color,
    note: String?,
    loading: Boolean,
    hasActions: Boolean,
    action: (@Composable () -> Unit)?,
    trailing: @Composable () -> Unit,
) {
    Row(
        modifier =
            Modifier
                .fillMaxWidth()
                .heightIn(min = 72.dp)
                .padding(start = 16.dp, end = if (hasActions) 4.dp else 16.dp, top = 12.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Box(
            modifier =
                Modifier
                    .size(40.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(MaterialTheme.colorScheme.surfaceContainerHigh),
            contentAlignment = Alignment.Center,
        ) {
            if (loading) {
                LoadingIndicator(modifier = Modifier.size(32.dp))
            } else {
                LucideIcon(
                    name = icon,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(20.dp),
                )
            }
        }
        Column(Modifier.weight(1f)) {
            Text(
                text = name,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.MiddleEllipsis,
            )
            Text(
                text = status,
                style = MaterialTheme.typography.bodyMedium,
                color = statusColor,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
            note?.let {
                Text(text = it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            action?.invoke()
        }
        trailing()
    }
}

@Composable
private fun FileMenu(
    name: String,
    enabled: Boolean,
    dateOrder: ImportDateOrder?,
    onDateOrder: (ImportDateOrder) -> Unit,
    onRemove: () -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }, enabled = enabled) {
            Icon(Icons.Filled.MoreVert, contentDescription = "Options for $name")
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            dateOrder?.let { order ->
                val other = if (order == ImportDateOrder.DAY_FIRST) ImportDateOrder.MONTH_FIRST else ImportDateOrder.DAY_FIRST
                DropdownMenuItem(
                    text = { Text(if (other == ImportDateOrder.MONTH_FIRST) "Read dates month first" else "Read dates day first") },
                    onClick = {
                        open = false
                        onDateOrder(other)
                    },
                )
            }
            DropdownMenuItem(
                text = { Text("Remove", color = MaterialTheme.colorScheme.error) },
                onClick = {
                    open = false
                    onRemove()
                },
            )
        }
    }
}

private fun sourcesTitle(
    hasFiles: Boolean,
    hasBankAccounts: Boolean,
    filesTitle: String,
): String =
    when {
        !hasBankAccounts -> filesTitle
        !hasFiles -> "Bank accounts"
        else -> "Sources"
    }

fun sourceLabel(source: ImportSource): String =
    when (val kind = source.kind) {
        is ImportSourceKind.File -> kind.file.name
        is ImportSourceKind.BankAccount -> bankAccountLabel(kind)
    }

private fun bankAccountLabel(account: ImportSourceKind.BankAccount): String =
    account.displayName?.takeIf(String::isNotBlank)?.let(::humanizeAccountName) ?: providerName(account.providerKind)

private fun providerName(kind: String): String = CONNECTORS.firstOrNull { it.kind == kind }?.name ?: kind

private fun fileStatusLine(
    file: ImportFile,
    recordedTransactions: Long,
): String {
    file.pausedUntil?.let {
        val formatter = DateTimeFormatter.ofPattern("d MMM, HH:mm", Locale.getDefault())
        return "Paused until ${Instant.ofEpochSecond(it).atZone(ZoneId.systemDefault()).format(formatter)}"
    }
    return when (file.status) {
        ImportFileStatus.PENDING -> "Waiting to be read"
        ImportFileStatus.PROCESSING -> "Reading…"
        ImportFileStatus.PARSED ->
            if (recordedTransactions > 0) {
                "Read · ${countOf(recordedTransactions, "transaction")} recorded"
            } else {
                "Read"
            }
        ImportFileStatus.FAILED -> listOfNotNull("Couldn't be read", file.error).joinToString(" · ")
    }
}

private fun dateOrderNote(order: ImportDateOrder): String =
    when (order) {
        ImportDateOrder.DAY_FIRST -> "Dates read day first, so 03/11 is 3 November"
        ImportDateOrder.MONTH_FIRST -> "Dates read month first, so 03/11 is 11 March"
    }

private fun fileGlyph(name: String): String =
    when (name.substringAfterLast('.', "").lowercase()) {
        "csv", "tsv", "xls", "xlsx", "ods", "numbers" -> "file-spreadsheet"
        "pdf" -> "file-text"
        "png", "jpg", "jpeg", "heic", "webp", "gif" -> "file-image"
        else -> "file"
    }
