package io.gannyu.input

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient

class TutorialActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val webView = WebView(this)
        webView.settings.apply {
            javaScriptEnabled = false
            domStorageEnabled = false
            allowFileAccess = false
            allowContentAccess = false
        }
        webView.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val uri = request.url
                if (uri == Uri.parse("https://github.com/Doohaey/GonnyuGeneralIME")) {
                    startActivity(Intent(Intent.ACTION_VIEW, uri))
                }
                return true
            }
        }
        val tutorial = assets.open("tutorial.html").bufferedReader().use { it.readText() }
        webView.loadDataWithBaseURL("https://appassets.androidplatform.net/", tutorial, "text/html", "UTF-8", null)
        setContentView(webView)
    }
}
