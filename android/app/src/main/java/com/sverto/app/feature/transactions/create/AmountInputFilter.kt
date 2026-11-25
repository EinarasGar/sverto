package com.sverto.app.feature.transactions.create

private val NUMERIC_PREFIX = Regex("""^\d*\.?\d*""")

internal fun filterAmountInput(input: String): String = NUMERIC_PREFIX.find(input)?.value.orEmpty()
