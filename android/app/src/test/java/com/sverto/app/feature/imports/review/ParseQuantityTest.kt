package com.sverto.app.feature.imports.review

import org.junit.Assert.assertEquals
import org.junit.Test
import java.text.DecimalFormatSymbols
import java.util.Locale

class ParseQuantityTest {
    private val english = DecimalFormatSymbols.getInstance(Locale.UK)
    private val german = DecimalFormatSymbols.getInstance(Locale.GERMANY)

    @Test
    fun readsGroupingAndDecimalSeparatorsOfTheLocale() {
        assertEquals("1234.56", parseQuantity("1,234.56", english))
        assertEquals("1234", parseQuantity("1,234", english))
        assertEquals("1234.56", parseQuantity("1.234,56", german))
        assertEquals("1.234", parseQuantity("1,234", german))
        assertEquals("-20.5", parseQuantity(" -20.5 ", english))
    }
}
