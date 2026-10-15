package com.sverto.app.core.update

enum class UpdateUrgency {
    NONE,
    BADGE,
    GRACE_NUDGE,
    NUDGE,
    PROMPT,
    REQUIRED,
    ;

    companion object {
        fun fromPriority(priority: Int): UpdateUrgency = entries[priority.coerceIn(0, entries.lastIndex)]
    }
}
