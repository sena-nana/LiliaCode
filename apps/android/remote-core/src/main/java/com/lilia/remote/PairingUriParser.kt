package com.lilia.remote

import java.net.URI
import java.net.URLDecoder

object PairingUriParser {
    private const val SUPPORTED_PROTOCOL_VERSION = 1
    private val SUPPORTED_SCHEMES = setOf("lilia-remote", "lilia-voice")

    fun parse(value: String): Result<RemotePairingTicket> = runCatching {
        val trimmed = value.trim()
        val uri = URI(trimmed)
        require(uri.scheme in SUPPORTED_SCHEMES) { "请输入 Lilia 配对链接" }
        require(uri.host == "pair") { "请输入 Lilia 配对链接" }
        val query = parseQuery(uri.rawQuery)
        val version = query["v"]?.toIntOrNull()
            ?: error("配对链接缺少协议版本")
        require(version == SUPPORTED_PROTOCOL_VERSION) {
            "不支持的远控协议版本：$version"
        }
        val ticket = query["ticket"]?.takeIf { it.isNotBlank() }
            ?: error("配对链接缺少票据")
        val challenge = query["challenge"]?.takeIf { it.isNotBlank() }
            ?: error("配对链接缺少验证信息")
        val endpoint = query["endpoint"]?.takeIf { it.isNotBlank() }
            ?: error("配对链接缺少电脑端点")
        val name = query["name"]?.takeIf { it.isNotBlank() } ?: DEFAULT_PC_DISPLAY_NAME
        val bridge = query["bridge"]?.takeIf { it.isNotBlank() }
            ?: error("配对链接缺少桥接地址")
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
            "配对桥接地址必须使用 HTTP(S)"
        }
        require(!bridgeUri.host.isNullOrBlank()) {
            "配对桥接地址缺少主机"
        }
        require(bridgeUri.port > 0) {
            "配对桥接地址缺少端口"
        }
        require(bridgeUri.rawPath.isNullOrBlank() || bridgeUri.rawPath == "/") {
            "配对桥接地址不能包含路径"
        }
        require(bridgeUri.rawQuery.isNullOrBlank() && bridgeUri.rawFragment.isNullOrBlank()) {
            "配对桥接地址不能包含查询参数或片段"
        }
    }
}
