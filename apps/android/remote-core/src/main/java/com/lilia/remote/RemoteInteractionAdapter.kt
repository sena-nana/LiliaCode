package com.lilia.remote

import org.json.JSONArray
import org.json.JSONObject

object RemoteInteractionAdapter {
    fun resultForInteraction(
        interaction: PendingInteraction,
        approve: Boolean,
        responseText: String? = null,
        selectedOptionIdsByQuestion: Map<String, List<String>> = emptyMap(),
    ): JSONObject {
        val payload = interaction.raw.optJSONObject("payload") ?: JSONObject()
        return when (interaction.kind) {
            "permission_approval" -> permissionApprovalResult(payload, approve)
            "tool_consent" -> JSONObject()
                .put("taskId", interaction.taskId)
                .put("requestId", interaction.requestId)
                .put("decision", if (approve) "allow" else "deny")
                .put("codexDecision", if (approve) "accept" else "decline")
                .put("message", JSONObject.NULL)
            "architecture_change" -> JSONObject()
                .put("decision", if (approve) "allow" else "deny")
                .put("message", JSONObject.NULL)
            "mcp_elicitation" -> mcpElicitationResult(approve)
            "ask_user", "plan_approval" -> askUserResult(
                interaction.kind,
                payload,
                approve,
                responseText,
                selectedOptionIdsByQuestion,
            )
            else -> JSONObject().put("action", if (approve) "approve" else "decline")
        }
    }

    private fun permissionApprovalResult(payload: JSONObject, approve: Boolean): JSONObject {
        val scope = payload.opt("scopeSuggestion") ?: "turn"
        val providerContext = payload.opt("providerContext") ?: JSONObject.NULL
        return if (approve) {
            JSONObject()
                .put("action", "approve")
                .put("grantedAccess", payload.opt("requestedAccess") ?: JSONObject())
                .put("scope", scope)
                .put("providerContext", providerContext)
        } else {
            JSONObject()
                .put("action", "decline")
                .put("grantedAccess", JSONObject())
                .put("scope", scope)
                .put("strictAutoReview", true)
                .put("providerContext", providerContext)
        }
    }

    private fun askUserResult(
        kind: String,
        payload: JSONObject,
        approve: Boolean,
        responseText: String?,
        selectedOptionIdsByQuestion: Map<String, List<String>>,
    ): JSONObject {
        val questions = payload.optJSONArray("questions") ?: JSONArray()
        val answers = JSONObject()
        for (index in 0 until questions.length()) {
            val question = questions.getJSONObject(index)
            val questionId = question.optString("id", "question-$index")
            answers.put(
                questionId,
                answerForQuestion(
                    kind,
                    questionId,
                    question,
                    approve,
                    responseText,
                    selectedOptionIdsByQuestion[questionId].orEmpty(),
                ),
            )
        }
        return JSONObject()
            .put("answers", answers)
            .put("cancelled", !approve)
    }

    private fun answerForQuestion(
        kind: String,
        questionId: String,
        question: JSONObject,
        approve: Boolean,
        responseText: String?,
        selectedOptionIds: List<String>,
    ): JSONObject {
        val trimmedResponse = responseText?.trim().orEmpty()
        if (!approve) {
            return JSONObject()
                .put("questionId", questionId)
                .put("value", "no")
        }
        val answer = JSONObject().put("questionId", questionId)
        val mode = question.optString("mode")
        val options = question.optJSONArray("options")
        if (selectedOptionIds.isNotEmpty() && options != null && options.length() > 0) {
            val hasOther = selectedOptionIds.contains("other")
            if (hasOther && question.optBoolean("allowOther")) {
                if (trimmedResponse.isNotEmpty()) {
                    val value = if (mode == "multi") {
                        selectedOptionIds.toJsonArray()
                    } else {
                        "other"
                    }
                    return answer
                        .put("value", value)
                        .put("notes", trimmedResponse)
                }
                val concreteOptions = selectedOptionIds.filter { it != "other" }
                if (concreteOptions.isEmpty()) {
                    return answer.put("value", answerValueForQuestion(question))
                }
                return answer.put(
                    "value",
                    if (mode == "multi") concreteOptions.toJsonArray() else concreteOptions.first(),
                )
            }
            return answer.put(
                "value",
                if (mode == "multi") selectedOptionIds.toJsonArray() else selectedOptionIds.first(),
            )
        }
        if (trimmedResponse.isNotEmpty() && mode == "confirm") {
            return if (kind == "plan_approval") {
                answer
                    .put("value", "revision_request")
                    .put("notes", trimmedResponse)
            } else {
                answer
                    .put("value", "no")
                    .put("notes", trimmedResponse)
            }
        }
        if (trimmedResponse.isNotEmpty() && (options == null || options.length() == 0)) {
            return answer.put("value", trimmedResponse)
        }
        if (trimmedResponse.isNotEmpty() && question.optBoolean("allowOther")) {
            if (mode == "multi") {
                return answer
                    .put("value", JSONArray().put("other"))
                    .put("notes", trimmedResponse)
            }
            return answer
                .put("value", "other")
                .put("notes", trimmedResponse)
        }
        return answer.put("value", answerValueForQuestion(question))
    }

    private fun List<String>.toJsonArray(): JSONArray {
        val array = JSONArray()
        forEach { array.put(it) }
        return array
    }

    private fun answerValueForQuestion(question: JSONObject): Any {
        val options = question.optJSONArray("options") ?: return "yes"
        val selected = recommendedOption(options) ?: options.optJSONObject(0)
        val value = selected?.optString("id")?.takeIf { it.isNotBlank() }
            ?: selected?.optString("label")?.takeIf { it.isNotBlank() }
            ?: "yes"
        return if (question.optString("mode") == "multi") {
            JSONArray().put(value)
        } else {
            value
        }
    }

    private fun recommendedOption(options: JSONArray): JSONObject? {
        for (index in 0 until options.length()) {
            val option = options.optJSONObject(index) ?: continue
            if (option.optBoolean("recommended")) return option
        }
        return null
    }

    private fun mcpElicitationResult(approve: Boolean): JSONObject {
        val result = JSONObject().put("action", if (approve) "accept" else "decline")
        if (approve) {
            result.put("content", JSONObject())
        }
        return result
    }
}
