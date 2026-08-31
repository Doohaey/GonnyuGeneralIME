package io.gannyu.input

import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.view.View
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.Spinner
import android.widget.TextView

class SetupActivity : Activity() {
    private var regions: List<GannyuInputMethodService.RegionOption> = emptyList()
    private lateinit var statusView: TextView
    private lateinit var loadingView: View
    private lateinit var loadingTextView: TextView
    private lateinit var regionSpinner: Spinner
    private lateinit var manageUserDataButton: Button
    private var busy = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_setup)

        statusView = findViewById(R.id.resourceStatus)
        loadingView = findViewById(R.id.regionLoading)
        loadingTextView = findViewById(R.id.regionLoadingText)
        regionSpinner = findViewById(R.id.regionSpinner)
        manageUserDataButton = findViewById(R.id.manageUserData)
        setupRegionPicker(statusView)
        manageUserDataButton.setOnClickListener { showClearChoices() }

        findViewById<Button>(R.id.openImeSettings).setOnClickListener {
            startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
        }
        findViewById<Button>(R.id.openImePicker).setOnClickListener {
            getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
        }
        findViewById<Button>(R.id.openTutorial).setOnClickListener {
            startActivity(Intent(this, TutorialActivity::class.java))
        }
    }

    private fun setupRegionPicker(statusView: TextView) {
        regions = GannyuInputMethodService.availableRegions()
        if (regions.isEmpty()) {
            statusView.setText(R.string.region_load_failed)
            regionSpinner.isEnabled = false
            findViewById<View>(R.id.regionSummary).visibility = View.GONE
            return
        }
        val currentId = GannyuInputMethodService.selectedRegionId(this) ?: regions.first().id
        GannyuInputMethodService.setSelectedRegionId(this, currentId)
        val labels = regions.map { it.displayLabel }
        val adapter = ArrayAdapter(this, android.R.layout.simple_spinner_item, labels)
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
        regionSpinner.adapter = adapter
        val currentIndex = regions.indexOfFirst { it.id == currentId }.takeIf { it >= 0 } ?: 0
        regionSpinner.setSelection(currentIndex, false)
        updateReadyState(regions[currentIndex].nameZh)
        maybePreload(regions[currentIndex], force = !GannyuInputMethodService.isRegionPrepared(currentId))
        regionSpinner.onItemSelectedListener = object : AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: AdapterView<*>?, view: View?, position: Int, id: Long) {
                val selected = regions.getOrNull(position) ?: return
                if (selected.id != GannyuInputMethodService.selectedRegionId(this@SetupActivity)) {
                    GannyuInputMethodService.setSelectedRegionId(this@SetupActivity, selected.id)
                    Log.i(TAG, "region switched to ${selected.id}")
                    maybePreload(selected, force = true)
                    return
                }
                maybePreload(selected, force = !GannyuInputMethodService.isRegionPrepared(selected.id))
            }

            override fun onNothingSelected(parent: AdapterView<*>?) = Unit
        }
    }

    private fun maybePreload(region: GannyuInputMethodService.RegionOption, force: Boolean) {
        if (!force) {
            updateReadyState(region.nameZh)
            return
        }
        setLoading(true, getString(R.string.region_loading, region.nameZh))
        GannyuInputMethodService.preloadSelectedRegionAsync(this, region.id) { success ->
            runOnUiThread {
                setLoading(false, "")
                if (success) {
                    updateReadyState(region.nameZh)
                } else {
                    statusView.text = getString(R.string.resource_failed_region, region.nameZh)
                }
            }
        }
    }

    private fun showClearChoices() {
        val labels = arrayOf(getString(R.string.clear_user_words), getString(R.string.clear_user_frequencies), getString(R.string.clear_all_user_data))
        AlertDialog.Builder(this).setTitle(R.string.clear_user_data_title).setItems(labels) { _, which ->
            val scope = intArrayOf(GannyuInputMethodService.USER_DATA_WORDS, GannyuInputMethodService.USER_DATA_FREQUENCIES, GannyuInputMethodService.USER_DATA_ALL)[which]
            val message = intArrayOf(R.string.clear_user_data_words_message, R.string.clear_user_data_frequencies_message, R.string.clear_user_data_all_message)[which]
            AlertDialog.Builder(this).setMessage(message).setNegativeButton(R.string.cancel_action, null).setPositiveButton(R.string.clear_action) { _, _ -> clearUserData(scope) }.show()
        }.show()
    }

    private fun clearUserData(scope: Int) {
        busy = true
        setLoading(true, getString(R.string.user_data_clearing))
        GannyuInputMethodService.clearUserDataAsync(this, scope) { success -> runOnUiThread {
            busy = false
            setLoading(false, "")
            statusView.text = getString(if (success) R.string.user_data_clear_success else R.string.user_data_clear_failed, if (scope == GannyuInputMethodService.USER_DATA_WORDS) getString(R.string.clear_user_words) else if (scope == GannyuInputMethodService.USER_DATA_FREQUENCIES) getString(R.string.clear_user_frequencies) else getString(R.string.clear_all_user_data))
        }}
    }

    private fun updateReadyState(regionName: String) {
        statusView.text = getString(R.string.resource_ready_region, regionName)
    }

    private fun setLoading(loading: Boolean, message: String) {
        loadingView.visibility = if (loading) View.VISIBLE else View.GONE
        loadingTextView.text = message
        regionSpinner.isEnabled = !loading && !busy
        manageUserDataButton.isEnabled = !loading && !busy
    }

    companion object {
        private const val TAG = "GannyuSetup"
    }
}
