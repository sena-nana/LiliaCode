package com.lilia.remote

import android.app.Application
import android.content.Context
import android.util.Log

class LiliaRemoteApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        installIrohAndroidContext(this)
    }

    private fun installIrohAndroidContext(context: Context) {
        runCatching {
            val irohAndroid = Class.forName("computer.iroh.IrohAndroid")
            val method = irohAndroid.methods.firstOrNull { candidate ->
                candidate.name == "installAndroidContext" &&
                    candidate.parameterTypes.size == 1 &&
                    Context::class.java.isAssignableFrom(candidate.parameterTypes[0])
            } ?: error("computer.iroh.IrohAndroid.installAndroidContext(Context) not found")

            method.invoke(null, context.applicationContext)
            Log.i("LiliaRemote", "iroh Android context installed")
        }.onFailure { error ->
            Log.w("LiliaRemote", "iroh Android context probe failed", error)
        }
    }
}
