package com.timspizza.swiftpan

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import android.os.Build
import android.app.Application
import android.webkit.WebView


class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            val processName = Application.getProcessName()
            if (applicationContext.packageName != processName) {
                WebView.setDataDirectorySuffix(processName)
            }
      }
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
