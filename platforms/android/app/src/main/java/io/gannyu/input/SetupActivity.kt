package io.gannyu.input

import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
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
import android.widget.Toast

class SetupActivity : Activity() {
    private var regions: List<GannyuInputMethodService.RegionOption> = emptyList()
    private lateinit var statusView: TextView
    private lateinit var loadingView: View
    private lateinit var loadingTextView: TextView
    private lateinit var regionSpinner: Spinner
    private lateinit var manageUserDataButton: Button
    private lateinit var openImePickerButton: Button
    private var busy = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_setup)

        statusView = findViewById(R.id.resourceStatus)
        loadingView = findViewById(R.id.regionLoading)
        loadingTextView = findViewById(R.id.regionLoadingText)
        regionSpinner = findViewById(R.id.regionSpinner)
        manageUserDataButton = findViewById(R.id.manageUserData)
        openImePickerButton = findViewById(R.id.openImePicker)
        setupRegionPicker(statusView)
        manageUserDataButton.setOnClickListener { showClearChoices() }

        findViewById<Button>(R.id.openImeSettings).setOnClickListener {
            showEnableImeInstructions()
        }
        openImePickerButton.setOnClickListener {
            getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
        }
        findViewById<Button>(R.id.openTutorial).setOnClickListener {
            startActivity(Intent(this, TutorialActivity::class.java))
        }
    }

    override fun onResume() {
        super.onResume()
        updateImeSetupState()
    }

    private fun showEnableImeInstructions() {
        AlertDialog.Builder(this)
            .setTitle(R.string.enable_ime_title)
            .setMessage(R.string.enable_ime_message)
            .setNegativeButton(R.string.cancel_action, null)
            .setPositiveButton(R.string.go_to_settings) { _, _ ->
                startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
            }
            .show()
    }

    private fun updateImeSetupState() {
        val inputMethodManager = getSystemService(InputMethodManager::class.java)
        val component = ComponentName(this, GannyuInputMethodService::class.java)
        val enabled = inputMethodManager?.enabledInputMethodList?.any {
            ComponentName.unflattenFromString(it.id) == component
        } == true
        openImePickerButton.isEnabled = enabled
    }

    private fun setupRegionPicker(statusView: TextView) {
        regions = GannyuInputMethodService.availableRegions(this)
        if (regions.isEmpty()) {
            statusView.setText(R.string.region_load_failed)
            statusView.visibility = View.VISIBLE
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
        updateReadyState()
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
            updateReadyState()
            return
        }
        setLoading(true, getString(R.string.region_loading, region.nameZh))
        GannyuInputMethodService.preloadSelectedRegionAsync(this, region.id) { success, detail ->
            runOnUiThread {
                setLoading(false, "")
                if (success) {
                    updateReadyState()
                } else {
                    statusView.text = getString(R.string.resource_failed_region, region.nameZh)
                    statusView.visibility = View.VISIBLE
                    showPreloadFailure(region, detail)
                }
            }
        }
    }

    private fun showPreloadFailure(region: GannyuInputMethodService.RegionOption, detail: String?) {
        val report = getString(
            R.string.resource_failed_detail,
            region.displayLabel,
            detail?.ifBlank { null } ?: getString(R.string.resource_failed_detail_missing),
        )
        AlertDialog.Builder(this)
            .setTitle(R.string.resource_failed_detail_title)
            .setMessage(report)
            .setNegativeButton(R.string.close_action, null)
            .setPositiveButton(R.string.copy_action) { _, _ ->
                getSystemService(ClipboardManager::class.java)?.setPrimaryClip(
                    ClipData.newPlainText("Gonnyu preload error", report),
                )
                Toast.makeText(this, R.string.copy_success, Toast.LENGTH_SHORT).show()
            }
            .show()
    }

    private fun showClearChoices() {
        val labels = arrayOf(
            getString(R.string.clear_current_region_user_data),
            getString(R.string.clear_all_region_user_data),
        )
        AlertDialog.Builder(this).setTitle(R.string.clear_user_data_title).setItems(labels) { _, which ->
            val target = intArrayOf(
                GannyuInputMethodService.USER_DATA_CURRENT_REGION,
                GannyuInputMethodService.USER_DATA_ALL_REGIONS,
            )[which]
            val message = intArrayOf(
                R.string.clear_current_region_user_data_message,
                R.string.clear_all_region_user_data_message,
            )[which]
            AlertDialog.Builder(this).setMessage(message).setNegativeButton(R.string.cancel_action, null).setPositiveButton(R.string.clear_action) { _, _ -> clearUserData(target) }.show()
        }.show()
    }

    private fun clearUserData(target: Int) {
        busy = true
        setLoading(true, getString(R.string.user_data_clearing))
        GannyuInputMethodService.clearUserDataAsync(this, target) { success -> runOnUiThread {
            busy = false
            setLoading(false, "")
            val targetLabel = getString(
                if (target == GannyuInputMethodService.USER_DATA_CURRENT_REGION) {
                    R.string.clear_current_region_user_data
                } else {
                    R.string.clear_all_region_user_data
                }
            )
            statusView.text = getString(
                if (success) R.string.user_data_clear_success else R.string.user_data_clear_failed,
                targetLabel,
            )
            statusView.visibility = View.VISIBLE
        }}
    }

    private fun updateReadyState() {
        statusView.visibility = View.GONE
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
