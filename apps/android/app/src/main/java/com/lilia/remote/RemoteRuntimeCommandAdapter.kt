package com.lilia.remote

import org.json.JSONObject

object RemoteRuntimeCommandAdapter {
    fun sessionFork(anchor: RemoteBranchAnchor): JSONObject =
        JSONObject()
            .put("type", "session_fork")
            .put("excludeTurns", true)
            .put("sourceTurnId", anchor.sourceTurnId)
            .put("mode", anchor.mode.wireValue)
}
