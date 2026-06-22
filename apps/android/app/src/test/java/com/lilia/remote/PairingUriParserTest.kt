package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PairingUriParserTest {
    @Test
    fun parsesPairingUriWithBridgeUrl() {
        val ticket = PairingUriParser.parse(
            " lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&name=Lilia%20Desktop&bridge=http%3A%2F%2F192.168.1.5%3A41478 ",
        ).getOrThrow()

        assertEquals(1, ticket.protocolVersion)
        assertEquals("ticket-1", ticket.ticketId)
        assertEquals("challenge-1", ticket.challenge)
        assertEquals("pc-1", ticket.endpointId)
        assertEquals("Lilia Desktop", ticket.pcName)
        assertEquals("http://192.168.1.5:41478", ticket.bridgeUrl)
    }

    @Test
    fun defaultsPcNameWhenNameIsMissing() {
        val ticket = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&bridge=http%3A%2F%2F192.168.1.5%3A41478",
        ).getOrThrow()

        assertEquals("Lilia PC", ticket.pcName)
    }

    @Test
    fun rejectsPairingUrisWithoutBridgeUrl() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1&endpoint=pc-1",
        )

        assertTrue(result.isFailure)
        assertEquals("Pairing URI missing bridge", result.exceptionOrNull()?.message)
    }

    @Test
    fun rejectsNonPairingUris() {
        val result = PairingUriParser.parse(
            "https://example.test/pair?v=1&ticket=ticket-1&challenge=challenge-1&endpoint=pc-1",
        )

        assertTrue(result.isFailure)
    }

    @Test
    fun rejectsUnsupportedProtocolVersions() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=2&ticket=ticket-1&challenge=challenge-1&endpoint=pc-1",
        )

        assertTrue(result.isFailure)
        assertEquals("Unsupported remote protocol version: 2", result.exceptionOrNull()?.message)
    }

    @Test
    fun rejectsNonHttpBridgeUrls() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&bridge=file%3A%2F%2F%2Ftmp%2Flilia",
        )

        assertTrue(result.isFailure)
        assertEquals("Pairing bridge must use HTTP(S)", result.exceptionOrNull()?.message)
    }

    @Test
    fun rejectsBridgeUrlsWithoutExplicitPort() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&bridge=http%3A%2F%2F192.168.1.5",
        )

        assertTrue(result.isFailure)
        assertEquals("Pairing bridge URL missing port", result.exceptionOrNull()?.message)
    }

    @Test
    fun rejectsBridgeUrlsWithPath() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&bridge=http%3A%2F%2F192.168.1.5%3A41478%2Fbridge",
        )

        assertTrue(result.isFailure)
        assertEquals("Pairing bridge URL must not include a path", result.exceptionOrNull()?.message)
    }

    @Test
    fun rejectsBridgeUrlsWithQueryOrFragment() {
        val result = PairingUriParser.parse(
            "lilia-remote://pair?v=1&ticket=ticket-1&challenge=challenge-1" +
                "&endpoint=pc-1&bridge=http%3A%2F%2F192.168.1.5%3A41478%3Fdebug%3D1",
        )

        assertTrue(result.isFailure)
        assertEquals("Pairing bridge URL must not include query or fragment", result.exceptionOrNull()?.message)
    }
}
