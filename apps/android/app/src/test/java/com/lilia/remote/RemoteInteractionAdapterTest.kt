package com.lilia.remote

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteInteractionAdapterTest {
    @Test
    fun permissionApprovalApproveGrantsRequestedAccessAndScope() {
        val interaction = pendingInteraction(
            kind = "permission_approval",
            payload = JSONObject()
                .put("requestedAccess", JSONObject().put("filesystem", "workspace-write"))
                .put("scopeSuggestion", "session")
                .put("providerContext", JSONObject().put("provider", "codex")),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(interaction, approve = true)

        assertEquals("approve", result.getString("action"))
        assertEquals("workspace-write", result.getJSONObject("grantedAccess").getString("filesystem"))
        assertEquals("session", result.getString("scope"))
        assertEquals("codex", result.getJSONObject("providerContext").getString("provider"))
        assertFalse(result.has("strictAutoReview"))
    }

    @Test
    fun permissionApprovalDeclineUsesStrictAutoReview() {
        val interaction = pendingInteraction(
            kind = "permission_approval",
            payload = JSONObject()
                .put("requestedAccess", JSONObject().put("filesystem", "workspace-write")),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(interaction, approve = false)

        assertEquals("decline", result.getString("action"))
        assertEquals(0, result.getJSONObject("grantedAccess").length())
        assertEquals("turn", result.getString("scope"))
        assertTrue(result.getBoolean("strictAutoReview"))
        assertTrue(result.has("providerContext"))
        assertTrue(result.isNull("providerContext"))
    }

    @Test
    fun permissionApprovalApprovePreservesNullProviderContext() {
        val interaction = pendingInteraction(
            kind = "permission_approval",
            payload = JSONObject()
                .put("requestedAccess", JSONObject().put("network", "enabled")),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(interaction, approve = true)

        assertEquals("approve", result.getString("action"))
        assertTrue(result.has("providerContext"))
        assertTrue(result.isNull("providerContext"))
    }

    @Test
    fun askUserApproveSelectsFirstOption() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choice",
                      "options": [
                        { "id": "recommended" },
                        { "id": "alternate" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(interaction, approve = true)

        assertFalse(result.getBoolean("cancelled"))
        assertEquals(
            "recommended",
            result.getJSONObject("answers").getJSONObject("choice").getString("value"),
        )
    }

    @Test
    fun askUserApprovePrefersRecommendedOptionAndPreservesMultiShape() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choices",
                      "mode": "multi",
                      "options": [
                        { "id": "first" },
                        { "id": "recommended", "recommended": true }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(interaction, approve = true)
        val value = result.getJSONObject("answers").getJSONObject("choices").getJSONArray("value")

        assertEquals(1, value.length())
        assertEquals("recommended", value.getString(0))
    }

    @Test
    fun askUserApproveUsesSelectedSingleOption() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choice",
                      "mode": "single",
                      "options": [
                        { "id": "recommended", "recommended": true },
                        { "id": "alternate" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            selectedOptionIdsByQuestion = mapOf("choice" to listOf("alternate")),
        )

        assertEquals(
            "alternate",
            result.getJSONObject("answers").getJSONObject("choice").getString("value"),
        )
    }

    @Test
    fun askUserApproveUsesSelectedMultiOptions() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choices",
                      "mode": "multi",
                      "options": [
                        { "id": "first" },
                        { "id": "second" },
                        { "id": "third" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            selectedOptionIdsByQuestion = mapOf("choices" to listOf("second", "third")),
        )
        val value = result.getJSONObject("answers").getJSONObject("choices").getJSONArray("value")

        assertEquals(2, value.length())
        assertEquals("second", value.getString(0))
        assertEquals("third", value.getString(1))
    }

    @Test
    fun askUserApproveUsesTypedResponseForQuestionWithoutOptions() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "reply",
                      "question": "What should I do next?"
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            responseText = "Continue with the Android MVP",
        )

        assertFalse(result.getBoolean("cancelled"))
        assertEquals(
            "Continue with the Android MVP",
            result.getJSONObject("answers").getJSONObject("reply").getString("value"),
        )
    }

    @Test
    fun askUserApproveUsesOtherNotesForAllowedOtherOptions() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choice",
                      "allowOther": true,
                      "options": [
                        { "id": "first" },
                        { "id": "second" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            responseText = "Use the phone flow",
        )
        val answer = result.getJSONObject("answers").getJSONObject("choice")

        assertEquals("other", answer.getString("value"))
        assertEquals("Use the phone flow", answer.getString("notes"))
    }

    @Test
    fun planApprovalTypedResponseUsesRevisionRequestNotes() {
        val interaction = pendingInteraction(
            kind = "plan_approval",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "approve-plan",
                      "mode": "confirm"
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            responseText = "Split the implementation into smaller steps",
        )
        val answer = result.getJSONObject("answers").getJSONObject("approve-plan")

        assertFalse(result.getBoolean("cancelled"))
        assertEquals("revision_request", answer.getString("value"))
        assertEquals("Split the implementation into smaller steps", answer.getString("notes"))
    }

    @Test
    fun confirmAskTypedResponseUsesNoWithNotes() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "confirm",
                      "mode": "confirm"
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            responseText = "Not until the device smoke passes",
        )
        val answer = result.getJSONObject("answers").getJSONObject("confirm")

        assertFalse(result.getBoolean("cancelled"))
        assertEquals("no", answer.getString("value"))
        assertEquals("Not until the device smoke passes", answer.getString("notes"))
    }

    @Test
    fun multiOtherTypedResponsePreservesArrayValue() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choices",
                      "mode": "multi",
                      "allowOther": true,
                      "options": [
                        { "id": "first" },
                        { "id": "second" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            responseText = "Use another path",
        )
        val answer = result.getJSONObject("answers").getJSONObject("choices")

        assertEquals("other", answer.getJSONArray("value").getString(0))
        assertEquals("Use another path", answer.getString("notes"))
    }

    @Test
    fun otherSelectionWithoutNotesFallsBackToConcreteOption() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choices",
                      "mode": "multi",
                      "allowOther": true,
                      "options": [
                        { "id": "first" },
                        { "id": "second" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            selectedOptionIdsByQuestion = mapOf("choices" to listOf("first", "other")),
        )
        val value = result.getJSONObject("answers").getJSONObject("choices").getJSONArray("value")

        assertEquals(1, value.length())
        assertEquals("first", value.getString(0))
    }

    @Test
    fun loneOtherSelectionWithoutNotesFallsBackToDefaultOption() {
        val interaction = pendingInteraction(
            kind = "ask_user",
            payload = JSONObject(
                """
                {
                  "questions": [
                    {
                      "id": "choice",
                      "mode": "single",
                      "allowOther": true,
                      "options": [
                        { "id": "first" },
                        { "id": "second" }
                      ]
                    }
                  ]
                }
                """.trimIndent(),
            ),
        )

        val result = RemoteInteractionAdapter.resultForInteraction(
            interaction,
            approve = true,
            selectedOptionIdsByQuestion = mapOf("choice" to listOf("other")),
        )

        assertEquals(
            "first",
            result.getJSONObject("answers").getJSONObject("choice").getString("value"),
        )
    }

    @Test
    fun mcpElicitationApproveIncludesEmptyContent() {
        val result = RemoteInteractionAdapter.resultForInteraction(
            pendingInteraction(kind = "mcp_elicitation", payload = JSONObject()),
            approve = true,
        )

        assertEquals("accept", result.getString("action"))
        assertEquals(0, result.getJSONObject("content").length())
    }

    @Test
    fun toolConsentIncludesGenericAndCodexDecisions() {
        val allowed = RemoteInteractionAdapter.resultForInteraction(
            pendingInteraction(kind = "tool_consent", payload = JSONObject(), includeRawTaskId = false),
            approve = true,
        )
        val denied = RemoteInteractionAdapter.resultForInteraction(
            pendingInteraction(kind = "tool_consent", payload = JSONObject()),
            approve = false,
        )

        assertEquals("allow", allowed.getString("decision"))
        assertEquals("accept", allowed.getString("codexDecision"))
        assertEquals("task-1", allowed.getString("taskId"))
        assertEquals("deny", denied.getString("decision"))
        assertEquals("decline", denied.getString("codexDecision"))
    }

    private fun pendingInteraction(
        kind: String,
        payload: JSONObject,
        includeRawTaskId: Boolean = true,
    ): PendingInteraction {
        val raw = JSONObject()
            .put("requestId", "request-1")
            .put("kind", kind)
            .put("backend", "codex")
            .put("payload", payload)
        if (includeRawTaskId) {
            raw.put("taskId", "task-1")
        }
        return PendingInteraction(
            taskId = "task-1",
            requestId = "request-1",
            kind = kind,
            backend = "codex",
            title = "Pending",
            body = "Body",
            approveLabel = "Approve",
            declineLabel = "Decline",
            raw = raw,
        )
    }
}
