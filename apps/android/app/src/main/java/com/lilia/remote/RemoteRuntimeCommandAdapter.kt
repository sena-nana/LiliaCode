package com.lilia.remote

import org.json.JSONObject

object RemoteRuntimeCommandAdapter {
    fun sessionFork(anchor: RemoteBranchAnchor): JSONObject =
        JSONObject()
            .put("type", "session_fork")
            .put("excludeTurns", true)
            .put("sourceTurnId", anchor.sourceTurnId)
            .put("mode", anchor.mode.wireValue)

    fun processSpawn(command: String): JSONObject =
        processSession("spawn").put("command", command)

    fun processWriteStdin(stdin: String): JSONObject =
        processSession("write_stdin").put("stdin", stdin)

    fun processKill(): JSONObject =
        processSession("kill")

    private fun processSession(action: String): JSONObject =
        JSONObject()
            .put("type", "process_session")
            .put("action", action)
}
