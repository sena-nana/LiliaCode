package com.lilia.remote

import org.junit.Assert.assertEquals
import org.junit.Test

class RemoteHttpResponseAdapterTest {
    @Test
    fun parsesJsonResponse() {
        val response = RemoteHttpResponseAdapter.parseJson(200, """{ "ok": true }""")

        assertEquals(true, response.getBoolean("ok"))
    }

    @Test
    fun rejectsEmptyResponseWithStatusCode() {
        val err = runCatching {
            RemoteHttpResponseAdapter.parseJson(502, "   ")
        }.exceptionOrNull()

        assertEquals("Remote bridge returned an empty response (HTTP 502).", err?.message)
    }

    @Test
    fun rejectsNonJsonResponseWithCompactPreview() {
        val err = runCatching {
            RemoteHttpResponseAdapter.parseJson(404, "<html>\n  <body>Not Found</body>\n</html>")
        }.exceptionOrNull()

        assertEquals(
            "Remote bridge returned a non-JSON response (HTTP 404): <html> <body>Not Found</body> </html>",
            err?.message,
        )
    }
}
