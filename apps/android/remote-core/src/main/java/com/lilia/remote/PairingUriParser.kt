package com.lilia.remote

import java.net.URI
import java.net.URLDecoder

object PairingUriParser {
    private const val SUPPORTED_PROTOCOL_VERSION = 1
    private val SUPPORTED_SCHEMES = setOf("lilia-remote", "lilia-voice")

    fun parse(value: String): Result<RemotePairingTicket> = runCatching {
        val trimmed = value.trim()
        val uri = URI(trimmed)
        require(uri.scheme in SUPPORTED_SCHEMES) { "Expected Lilia pairing URI" }
        require(uri.host == "pair") { "Expected Lilia pairing URI" }
        val query = parseQuery(uri.rawQuery)
        val version = query["v"]?.toIntOrNull()
            ?: error("Pairing URI missing protocol version")
        require(version == SUPPORTED_PROTOCOL_VERSION) {
            "Unsupported remote protocol version: $version"
        }
        val ticket = query["ticket"]?.takeIf { it.isNotBlank() }
            ?: error("Pairing URI missing ticket")
        val challenge = query["challenge"]?.takeIf { it.isNotBlank() }
            ?: error("Pairing URI missing challenge")
        val endpoint = query["endpoint"]?.takeIf { it.isNotBlank() }
            ?: error("Pairing URI missing endpoint")
        val name = query["name"]?.takeIf { it.isNotBlank() } ?: "Lilia PC"
        val bridge = query["bridge"]?.takeIf { it.isNotBlank() }
            ?: error("Pairing URI missing bridge")
        validateBridgeUrl(bridge)
        RemotePairingTicket(
            protocolVersion = version,
            ticketId = ticket,
            challenge = challenge,
            endpointId = endpoint,
            pcName = name,
            bridgeUrl = bridge,
            rawUri = trimmed,
        )
    }

    private fun parseQuery(rawQuery: String?): Map<String, String> {
        if (rawQuery.isNullOrBlank()) return emptyMap()
        return buildMap {
            rawQuery.split('&')
                .filter { it.isNotBlank() }
                .forEach { pair ->
                    val parts = pair.split('=', limit = 2)
                    val key = decode(parts[0])
                    if (key.isNotBlank()) {
                        put(key, decode(parts.getOrElse(1) { "" }))
                    }
                }
        }
    }

    private fun decode(value: String): String =
        URLDecoder.decode(value, Charsets.UTF_8.name())

    private fun validateBridgeUrl(value: String) {
        val bridgeUri = URI(value)
        val scheme = bridgeUri.scheme?.lowercase()
        require(scheme == "http" || scheme == "https") {
            "Pairing bridge must use HTTP(S)"
        }
        require(!bridgeUri.host.isNullOrBlank()) {
            "Pairing bridge URL missing host"
        }
        require(bridgeUri.port > 0) {
            "Pairing bridge URL missing port"
        }
        require(bridgeUri.rawPath.isNullOrBlank() || bridgeUri.rawPath == "/") {
            "Pairing bridge URL must not include a path"
        }
        require(bridgeUri.rawQuery.isNullOrBlank() && bridgeUri.rawFragment.isNullOrBlank()) {
            "Pairing bridge URL must not include query or fragment"
        }
    }
}
