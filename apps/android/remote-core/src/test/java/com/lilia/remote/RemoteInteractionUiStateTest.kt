package com.lilia.remote

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteInteractionUiStateTest {
    @Test
    fun optionQuestionReadsMultiSelectionBoundsAndOtherOption() {
        val question = interactionOptionQuestion(
            pendingInteraction(
                """
                {
                  "questions": [
                    {
                      "id": "choices",
                      "mode": "multi",
                      "minSelections": 2,
                      "maxSelections": 3,
                      "allowOther": true,
                      "options": [
                        { "id": "first", "label": "First" },
                        { "id": "second", "label": "Second" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        assertNotNull(question)
        assertEquals("choices", question?.questionId)
        assertEquals("multi", question?.mode)
        assertEquals(2, question?.minSelections)
        assertEquals(3, question?.maxSelections)
        assertEquals(listOf("first", "second", "other"), question?.options?.map { it.id })
    }

    @Test
    fun interactionSubmitRequiresMinimumSelections() {
        val question = InteractionOptionQuestion(
            questionId = "choices",
            mode = "multi",
            minSelections = 2,
            maxSelections = 3,
            options = listOf(
                InteractionOption("first", "First", null),
                InteractionOption("second", "Second", null),
                InteractionOption("third", "Third", null),
            ),
        )

        assertFalse(interactionCanSubmit(question, setOf("first"), ""))
        assertTrue(interactionCanSubmit(question, setOf("first", "second"), ""))
    }

    @Test
    fun interactionSubmitRejectsMoreThanMaximumSelections() {
        val question = InteractionOptionQuestion(
            questionId = "choices",
            mode = "multi",
            minSelections = 1,
            maxSelections = 2,
            options = listOf(
                InteractionOption("first", "First", null),
                InteractionOption("second", "Second", null),
                InteractionOption("third", "Third", null),
            ),
        )

        assertTrue(interactionCanSubmit(question, setOf("first", "second"), ""))
        assertFalse(interactionCanSubmit(question, setOf("first", "second", "third"), ""))
    }

    @Test
    fun interactionSubmitRequiresTextForOtherSelection() {
        val question = InteractionOptionQuestion(
            questionId = "choice",
            mode = "single",
            minSelections = 1,
            maxSelections = null,
            options = listOf(
                InteractionOption("first", "First", null),
                InteractionOption("other", "Other", null),
            ),
        )

        assertFalse(interactionCanSubmit(question, setOf("other"), ""))
        assertTrue(interactionCanSubmit(question, setOf("other"), "Use another path"))
    }

    @Test
    fun selectedOptionsPreservesQuestionOptionOrder() {
        val question = InteractionOptionQuestion(
            questionId = "choices",
            mode = "multi",
            minSelections = 1,
            maxSelections = null,
            options = listOf(
                InteractionOption("first", "First", null),
                InteractionOption("second", "Second", null),
                InteractionOption("third", "Third", null),
            ),
        )

        assertEquals(
            mapOf("choices" to listOf("first", "third")),
            selectedOptionsByQuestion(question, setOf("third", "first")),
        )
    }

    @Test
    fun taskInboxUnsupportedMessageIsActionable() {
        assertEquals(
            "Task inbox is not supported by this PC bridge.",
            taskInboxUnsupportedMessage(),
        )
    }

    private fun pendingInteraction(payload: String): PendingInteraction = PendingInteraction(
        taskId = "task-1",
        requestId = "request-1",
        kind = "ask_user",
        backend = "codex",
        title = "Question",
        body = "Choose",
        approveLabel = "Answer",
        declineLabel = "Decline",
        raw = JSONObject()
            .put("requestId", "request-1")
            .put("kind", "ask_user")
            .put("payload", JSONObject(payload)),
    )
}
