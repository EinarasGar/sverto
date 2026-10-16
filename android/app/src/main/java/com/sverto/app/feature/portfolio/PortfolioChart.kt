package com.sverto.app.feature.portfolio

import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.ButtonGroupDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.ToggleButton
import androidx.compose.material3.ToggleButtonDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.sverto.app.core.Money
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

data class ChartPoint(
    val date: Long,
    val value: Double,
)

enum class TimePeriod(
    val label: String,
    val apiRange: String,
) {
    DAY("1D", "1d"),
    WEEK("1W", "1w"),
    MONTH("1M", "1m"),
    THREE_MONTHS("3M", "3m"),
    SIX_MONTHS("6M", "6m"),
    YEAR("1Y", "1y"),
    ALL("ALL", "all"),
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun PortfolioChart(
    portfolioData: Map<TimePeriod, List<ChartPoint>>,
    currencyTicker: String,
    modifier: Modifier = Modifier,
    selectedPeriod: TimePeriod? = null,
    onPeriodSelect: ((TimePeriod) -> Unit)? = null,
    headerTrailing: (@Composable () -> Unit)? = null,
) {
    var internalPeriod by remember { mutableStateOf(TimePeriod.MONTH) }
    val activePeriod = selectedPeriod ?: internalPeriod
    val periodPoints = portfolioData[activePeriod] ?: emptyList()
    val points = remember(periodPoints) { periodPoints.associateBy { it.date }.values.sortedBy { it.date } }

    val scrubPosition = remember(activePeriod) { mutableStateOf<Float?>(null) }
    val scrubIdx by remember(points, scrubPosition) {
        derivedStateOf {
            scrubPosition.value?.let { nearestChartPointIndex(points, it) }
        }
    }
    val dateFormat =
        remember(activePeriod) {
            SimpleDateFormat(if (activePeriod == TimePeriod.DAY) "MMM d, h:mm a" else "MMM d, yyyy", Locale.US)
        }

    val scrubbedPoint = scrubIdx?.let { points[it] }
    val currentValue = scrubbedPoint?.value ?: points.lastOrNull()?.value ?: 0.0
    val startValue = points.firstOrNull()?.value ?: 0.0
    val changeAmount = currentValue - startValue
    val changePercent = if (startValue != 0.0) (changeAmount / startValue) * 100 else 0.0
    val isPositive = changeAmount >= 0

    val scrubDate = scrubbedPoint?.let { dateFormat.format(Date(it.date * 1000)) }

    val lineColor =
        if (isPositive) {
            MaterialTheme.colorScheme.primary
        } else {
            MaterialTheme.colorScheme.error
        }
    val fillColor =
        if (isPositive) {
            MaterialTheme.colorScheme.primaryContainer
        } else {
            MaterialTheme.colorScheme.errorContainer
        }
    val changeColor =
        if (isPositive) {
            MaterialTheme.colorScheme.primary
        } else {
            MaterialTheme.colorScheme.error
        }

    // Reset in composition so a new period never flashes its complete path before the reveal.
    val animationProgress = remember(activePeriod) { Animatable(0f) }
    LaunchedEffect(activePeriod) {
        animationProgress.animateTo(1f, animationSpec = tween(600))
    }

    Column(modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.Top,
        ) {
            Column(Modifier.weight(1f)) {
                ChartHeadlineValue(
                    text = Money.format(currentValue, currencyTicker),
                )
                val changeText = Money.format(changeAmount, currencyTicker, signed = true)
                val pctText = String.format(Locale.US, "%.2f", changePercent)
                val pctSign = if (isPositive) "+" else ""
                Text(
                    text = "$changeText ($pctSign$pctText%)",
                    style = MaterialTheme.typography.bodyMedium,
                    color = changeColor,
                )
                // Reserve the date line so pressing the chart does not move the plot.
                Text(
                    text = scrubDate ?: " ",
                    style = MaterialTheme.typography.bodySmall,
                    color =
                        if (scrubDate != null) {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        } else {
                            Color.Transparent
                        },
                    maxLines = 1,
                )
            }
            headerTrailing?.invoke()
        }
        Spacer(Modifier.height(16.dp))

        key(activePeriod) {
            PortfolioLineCanvas(
                points = points,
                progress = animationProgress.asState(),
                scrubPosition = scrubPosition,
                lineColor = lineColor,
                fillColor = fillColor,
                onScrub = { scrubPosition.value = it },
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(180.dp),
            )
        }

        Spacer(Modifier.height(16.dp))

        Row(
            horizontalArrangement = Arrangement.spacedBy(ButtonGroupDefaults.ConnectedSpaceBetween),
            modifier = Modifier.fillMaxWidth(),
        ) {
            val periods = TimePeriod.entries
            val leadingShapes = ButtonGroupDefaults.connectedLeadingButtonShapes()
            val middleShapes = ButtonGroupDefaults.connectedMiddleButtonShapes()
            val trailingShapes = ButtonGroupDefaults.connectedTrailingButtonShapes()
            val periodColors =
                ToggleButtonDefaults.toggleButtonColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                )
            val periodContentPadding = PaddingValues(horizontal = 6.dp, vertical = 10.dp)
            periods.forEachIndexed { index, period ->
                ToggleButton(
                    checked = activePeriod == period,
                    onCheckedChange = {
                        if (onPeriodSelect != null) {
                            onPeriodSelect(period)
                        } else {
                            internalPeriod = period
                        }
                        scrubPosition.value = null
                    },
                    modifier =
                        Modifier
                            .weight(1f)
                            .semantics { role = Role.RadioButton },
                    shapes =
                        when (index) {
                            0 -> leadingShapes
                            periods.lastIndex -> trailingShapes
                            else -> middleShapes
                        },
                    colors = periodColors,
                    contentPadding = periodContentPadding,
                ) {
                    Text(period.label, maxLines = 1)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ChartHeadlineValue(text: String) {
    var lastText by remember { mutableStateOf(text) }
    val displacement = remember { Animatable(0f) }
    val spatialSpec = MaterialTheme.motionScheme.fastSpatialSpec<Float>()

    LaunchedEffect(text) {
        if (text == lastText) return@LaunchedEffect
        lastText = text
        displacement.snapTo(1f)
        displacement.animateTo(0f, spatialSpec)
    }

    Text(
        text = text,
        style = MaterialTheme.typography.headlineLarge,
        modifier =
            Modifier
                .animateContentSize(MaterialTheme.motionScheme.fastSpatialSpec())
                .graphicsLayer { translationY = 2.dp.toPx() * displacement.value },
    )
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun PortfolioLineCanvas(
    points: List<ChartPoint>,
    progress: State<Float>,
    scrubPosition: State<Float?>,
    lineColor: Color,
    fillColor: Color,
    onScrub: (Float?) -> Unit,
    modifier: Modifier = Modifier,
) {
    val currentOnScrub by rememberUpdatedState(onScrub)
    val currentLineColor = rememberUpdatedState(lineColor)
    val currentFillColor = rememberUpdatedState(fillColor)
    val scrubLineColor = rememberUpdatedState(MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.3f))
    val lastScrubPosition = remember { mutableFloatStateOf(0f) }
    val isDragging = remember { mutableStateOf(false) }
    val isScrubbing by remember(scrubPosition) { derivedStateOf { scrubPosition.value != null } }
    val scrubOpacity =
        animateFloatAsState(
            targetValue = if (isScrubbing) 1f else 0f,
            animationSpec = MaterialTheme.motionScheme.fastEffectsSpec(),
            label = "chart_selection_opacity",
        )
    val scrubScale =
        animateFloatAsState(
            targetValue = if (isScrubbing) 1f else 0.7f,
            animationSpec = MaterialTheme.motionScheme.fastSpatialSpec(),
            label = "chart_selection_scale",
        )
    val updateScrub =
        remember {
            { position: Float? ->
                if (position != null) lastScrubPosition.floatValue = position
                currentOnScrub(position)
            }
        }
    val chartDrawing =
        remember(points, progress, scrubPosition) {
            Modifier.drawWithCache {
                if (points.size < 2) return@drawWithCache onDrawBehind {}

                val maxVal = points.maxOf { it.value }
                val minVal = points.minOf { it.value }
                val dataRange = maxVal - minVal
                val midVal = (maxVal + minVal) / 2.0
                val minRange = kotlin.math.abs(midVal) * 0.02
                val range = dataRange.coerceAtLeast(minRange).coerceAtLeast(1.0)
                val chartMin = midVal - range / 2.0
                val topPad = 8.dp.toPx()
                val chartHeight = size.height - topPad - 8.dp.toPx()
                val startDate = points.first().date
                val duration = (points.last().date - startDate).toDouble().coerceAtLeast(1.0)
                val coords =
                    points.map { point ->
                        Offset(
                            x = ((point.date - startDate) / duration).toFloat() * size.width,
                            y = topPad + chartHeight * (1f - ((point.value - chartMin) / range).toFloat()),
                        )
                    }
                val linePath = buildSmoothPath(coords)
                val fillPath =
                    Path().apply {
                        addPath(linePath)
                        lineTo(coords.last().x, topPad + chartHeight)
                        lineTo(coords.first().x, topPad + chartHeight)
                        close()
                    }
                val stroke = Stroke(width = 2.5.dp.toPx(), cap = StrokeCap.Round, join = StrokeJoin.Round)
                var gradientColor = Color.Unspecified
                var fillBrush: Brush? = null

                onDrawBehind {
                    val currentFill = currentFillColor.value
                    if (currentFill != gradientColor) {
                        gradientColor = currentFill
                        fillBrush =
                            Brush.verticalGradient(
                                colors = listOf(currentFill.copy(alpha = 0.5f), currentFill.copy(alpha = 0f)),
                                startY = topPad,
                                endY = topPad + chartHeight,
                            )
                    }
                    clipRect(right = size.width * progress.value.coerceIn(0f, 1f)) {
                        fillBrush?.let { drawPath(path = fillPath, brush = it) }
                        drawPath(path = linePath, color = currentLineColor.value, style = stroke)
                    }

                    val opacity = scrubOpacity.value
                    if (opacity > 0f) {
                        val position = scrubPosition.value ?: lastScrubPosition.floatValue
                        val idx = nearestChartPointIndex(points, position) ?: return@onDrawBehind
                        val center = coords[idx]
                        drawLine(
                            color = scrubLineColor.value,
                            start = Offset(center.x, topPad),
                            end = Offset(center.x, topPad + chartHeight),
                            strokeWidth = 1.dp.toPx(),
                            alpha = opacity,
                        )
                        val scale = scrubScale.value
                        drawCircle(color = Color.White, radius = 6.dp.toPx() * scale, center = center, alpha = opacity)
                        drawCircle(color = currentLineColor.value, radius = 4.dp.toPx() * scale, center = center, alpha = opacity)
                    }
                }
            }
        }

    Box(
        modifier
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragStart = { offset ->
                        isDragging.value = true
                        updateScrub(offset.x / size.width)
                    },
                    onDragEnd = {
                        isDragging.value = false
                        updateScrub(null)
                    },
                    onDragCancel = {
                        isDragging.value = false
                        updateScrub(null)
                    },
                    onHorizontalDrag = { change, _ ->
                        change.consume()
                        updateScrub((change.position.x / size.width).coerceIn(0f, 1f))
                    },
                )
            }.pointerInput(Unit) {
                detectTapGestures(
                    onPress = { offset ->
                        updateScrub(offset.x / size.width)
                        val released = tryAwaitRelease()
                        if (released || !isDragging.value) updateScrub(null)
                    },
                )
            }.then(chartDrawing),
    )
}

private fun nearestChartPointIndex(
    points: List<ChartPoint>,
    position: Float,
): Int? {
    if (points.isEmpty()) return null
    if (points.size == 1) return 0

    val startDate = points.first().date
    val duration = points.last().date - startDate
    if (duration <= 0) return points.lastIndex

    val targetDate = startDate.toDouble() + position.coerceIn(0f, 1f) * duration.toDouble()
    val index = points.binarySearch { it.date.toDouble().compareTo(targetDate) }
    if (index >= 0) return index

    val nextIndex = -index - 1
    if (nextIndex == 0) return 0
    if (nextIndex == points.size) return points.lastIndex

    val previousIndex = nextIndex - 1
    return if (targetDate - points[previousIndex].date <= points[nextIndex].date - targetDate) {
        previousIndex
    } else {
        nextIndex
    }
}

private fun buildSmoothPath(points: List<Offset>): Path {
    val path = Path()
    if (points.isEmpty()) return path

    path.moveTo(points[0].x, points[0].y)
    for (i in 1 until points.size) {
        val prev = points[i - 1]
        val curr = points[i]
        val cpx = (prev.x + curr.x) / 2f
        if (points.size == 2) {
            path.lineTo(curr.x, curr.y)
        } else {
            path.cubicTo(cpx, prev.y, cpx, curr.y, curr.x, curr.y)
        }
    }
    return path
}
