//
//  HarfiySettings.swift
//  Harfiy
//
//  Shared settings and persistent statistics between HarfiyApp and HarfiyKeyboard.
//

import Foundation
import Combine
#if canImport(UIKit)
import UIKit
#endif

public final class HarfiySettings: ObservableObject {
    public static let shared = HarfiySettings()

    public static let appGroupIdentifier = "group.uz.harfiy.keyboard"

    private let userDefaults: UserDefaults

    // Setting keys matching Android
    public static let keyInstantUndo = "instant_undo"
    public static let keyEnableNg = "enable_ng"
    public static let keySmartApostrophe = "smart_apostrophe"
    public static let keyHapticFeedback = "haptic_feedback"
    public static let keyAudioFeedback = "audio_feedback"
    public static let keyHasCompletedWizard = "has_completed_wizard"

    // Keyboard activation keys
    public static let keyKeyboardExtensionActive = "is_keyboard_extension_active"

    // Statistics keys
    public static let keyStatsTotal = "stats_total_conversions"
    public static let keyStatsSh = "stats_sh_count"
    public static let keyStatsCh = "stats_ch_count"
    public static let keyStatsO = "stats_o_count"
    public static let keyStatsG = "stats_g_count"
    public static let keyStatsNg = "stats_ng_count"

    /// Unified check across iOS system preferences, App Group defaults, and active input modes
    public static func isKeyboardActiveInSystem() -> Bool {
        let bundlePrefix = "uz.harfiy.keyboard"

        // 1. Check shared group defaults if keyboard extension ever ran
        if let groupDefaults = UserDefaults(suiteName: appGroupIdentifier),
           groupDefaults.bool(forKey: keyKeyboardExtensionActive) {
            return true
        }

        // 2. Check AppleKeyboards in standard defaults
        if let keyList = UserDefaults.standard.object(forKey: "AppleKeyboards") as? [String],
           keyList.contains(where: { $0.contains(bundlePrefix) }) {
            return true
        }

        #if canImport(UIKit)
        // 3. Check activeInputModes
        for mode in UITextInputMode.activeInputModes {
            if let id = mode.value(forKey: "identifier") as? String, id.contains(bundlePrefix) {
                return true
            }
            if let lang = mode.primaryLanguage, lang.contains(bundlePrefix) {
                return true
            }
        }
        #endif

        // On device these are sandboxed and never succeed, so skip the main-thread disk reads.
        #if targetEnvironment(simulator)
        // 4. Simulator only: check com.apple.keyboard.preferences (iOS system preference suite)
        if let kbPrefs = UserDefaults(suiteName: "com.apple.keyboard.preferences") {
            if let list = kbPrefs.object(forKey: "KeyboardsCurrentAndNext") as? [String],
               list.contains(where: { $0.contains(bundlePrefix) }) {
                return true
            }
            if let lastUsed = kbPrefs.string(forKey: "KeyboardLastUsed"),
               lastUsed.contains(bundlePrefix) {
                return true
            }
            if let langDict = kbPrefs.dictionary(forKey: "KeyboardLastUsedForLanguage") as? [String: String],
               langDict.values.contains(where: { $0.contains(bundlePrefix) }) {
                return true
            }
        }

        // Check simulator preference files directly
        let prefPaths = [
            NSHomeDirectory() + "/../../Library/Preferences/com.apple.keyboard.preferences.plist",
            NSHomeDirectory() + "/../Library/Preferences/com.apple.keyboard.preferences.plist",
            "/var/mobile/Library/Preferences/com.apple.keyboard.preferences.plist"
        ]
        for path in prefPaths {
            if let dict = NSDictionary(contentsOfFile: path) {
                if let list = dict["KeyboardsCurrentAndNext"] as? [String],
                   list.contains(where: { $0.contains(bundlePrefix) }) {
                    return true
                }
                if let lastUsed = dict["KeyboardLastUsed"] as? String,
                   lastUsed.contains(bundlePrefix) {
                    return true
                }
                if let langDict = dict["KeyboardLastUsedForLanguage"] as? [String: String],
                   langDict.values.contains(where: { $0.contains(bundlePrefix) }) {
                    return true
                }
            }
        }
        #endif

        return false
    }

    public init() {
        if let groupDefaults = UserDefaults(suiteName: HarfiySettings.appGroupIdentifier) {
            self.userDefaults = groupDefaults
        } else {
            self.userDefaults = UserDefaults.standard
        }

        // Register default values
        userDefaults.register(defaults: [
            Self.keyInstantUndo: true,
            Self.keyEnableNg: false,
            Self.keySmartApostrophe: true,
            Self.keyHapticFeedback: true,
            Self.keyAudioFeedback: true,
            Self.keyHasCompletedWizard: false
        ])

        self.instantUndo = userDefaults.bool(forKey: Self.keyInstantUndo)
        self.enableNg = userDefaults.bool(forKey: Self.keyEnableNg)
        self.smartApostrophe = userDefaults.bool(forKey: Self.keySmartApostrophe)
        self.hapticFeedback = userDefaults.bool(forKey: Self.keyHapticFeedback)
        self.audioFeedback = userDefaults.bool(forKey: Self.keyAudioFeedback)
        self.hasCompletedWizard = userDefaults.bool(forKey: Self.keyHasCompletedWizard)

        self.totalConversions = userDefaults.integer(forKey: Self.keyStatsTotal)
        self.shCount = userDefaults.integer(forKey: Self.keyStatsSh)
        self.chCount = userDefaults.integer(forKey: Self.keyStatsCh)
        self.oCount = userDefaults.integer(forKey: Self.keyStatsO)
        self.gCount = userDefaults.integer(forKey: Self.keyStatsG)
        self.ngCount = userDefaults.integer(forKey: Self.keyStatsNg)
    }

    /// Set while `reloadSettings()` runs so the `didSet` observers don't write the values back.
    private var isReloading = false

    private func persist(_ value: Bool, forKey key: String) {
        guard !isReloading else { return }
        userDefaults.set(value, forKey: key)
    }

    /// Flag (once) that the keyboard extension has run, so the app can detect it is enabled.
    public func markKeyboardExtensionActive() {
        guard !userDefaults.bool(forKey: Self.keyKeyboardExtensionActive) else { return }
        userDefaults.set(true, forKey: Self.keyKeyboardExtensionActive)
    }

    /// Reload user preferences from shared UserDefaults (e.g. when keyboard appears)
    public func reloadSettings() {
        isReloading = true
        defer { isReloading = false }
        self.instantUndo = userDefaults.bool(forKey: Self.keyInstantUndo)
        self.enableNg = userDefaults.bool(forKey: Self.keyEnableNg)
        self.smartApostrophe = userDefaults.bool(forKey: Self.keySmartApostrophe)
        self.hapticFeedback = userDefaults.bool(forKey: Self.keyHapticFeedback)
        self.audioFeedback = userDefaults.bool(forKey: Self.keyAudioFeedback)
        self.hasCompletedWizard = userDefaults.bool(forKey: Self.keyHasCompletedWizard)
    }

    @Published public var instantUndo: Bool {
        didSet { persist(instantUndo, forKey: Self.keyInstantUndo) }
    }

    @Published public var enableNg: Bool {
        didSet { persist(enableNg, forKey: Self.keyEnableNg) }
    }

    @Published public var smartApostrophe: Bool {
        didSet { persist(smartApostrophe, forKey: Self.keySmartApostrophe) }
    }

    @Published public var hapticFeedback: Bool {
        didSet { persist(hapticFeedback, forKey: Self.keyHapticFeedback) }
    }

    @Published public var audioFeedback: Bool {
        didSet { persist(audioFeedback, forKey: Self.keyAudioFeedback) }
    }

    @Published public var hasCompletedWizard: Bool {
        didSet { persist(hasCompletedWizard, forKey: Self.keyHasCompletedWizard) }
    }

    // Statistics
    @Published public var totalConversions: Int = 0
    @Published public var shCount: Int = 0
    @Published public var chCount: Int = 0
    @Published public var oCount: Int = 0
    @Published public var gCount: Int = 0
    @Published public var ngCount: Int = 0

    public var timeSavedSeconds: Double {
        // Average 0.2 seconds saved per auto-corrected character
        return Double(totalConversions) * 0.2
    }

    public func reloadStats() {
        totalConversions = userDefaults.integer(forKey: Self.keyStatsTotal)
        shCount = userDefaults.integer(forKey: Self.keyStatsSh)
        chCount = userDefaults.integer(forKey: Self.keyStatsCh)
        oCount = userDefaults.integer(forKey: Self.keyStatsO)
        gCount = userDefaults.integer(forKey: Self.keyStatsG)
        ngCount = userDefaults.integer(forKey: Self.keyStatsNg)
    }

    // MARK: - Statistics writes

    /// Conversions made in the keyboard are counted in memory and written in batches:
    /// every UserDefaults write is a cfprefsd round-trip, too heavy to pay per keystroke.
    private struct PendingStats {
        var total = 0, sh = 0, ch = 0, o = 0, g = 0, ng = 0

        mutating func add(character: String) {
            total += 1
            let lower = character.lowercased()
            if lower.contains("ş") {
                sh += 1
            } else if lower.contains("ç") {
                ch += 1
            } else if lower.contains("ö") || lower.contains("oʻ") || lower.contains("o'") {
                o += 1
            } else if lower.contains("ğ") || lower.contains("gʻ") || lower.contains("g'") {
                g += 1
            } else if lower.contains("ñ") {
                ng += 1
            }
        }
    }

    private static let statsFlushBatchSize = 20
    private static let statsFlushIdleDelay: TimeInterval = 3

    /// Serial queue guarding `pendingStats`, `idleFlush` and every stats write made by the keyboard.
    private let statsQueue = DispatchQueue(label: "uz.harfiy.keyboard.stats", qos: .utility)
    private var pendingStats = PendingStats()
    private var idleFlush: DispatchWorkItem?

    /// Count one automatic conversion. Never blocks the caller.
    public func recordConversion(character: String) {
        statsQueue.async {
            self.pendingStats.add(character: character)
            if self.pendingStats.total >= Self.statsFlushBatchSize {
                self.flushPendingStatsLocked()
            } else {
                self.scheduleIdleFlushLocked()
            }
        }
    }

    /// Write any buffered conversions now (call when the keyboard goes away).
    public func flushPendingStats() {
        statsQueue.async(qos: .userInitiated) {
            self.flushPendingStatsLocked()
        }
    }

    private func scheduleIdleFlushLocked() {
        idleFlush?.cancel()
        let item = DispatchWorkItem { [weak self] in self?.flushPendingStatsLocked() }
        idleFlush = item
        statsQueue.asyncAfter(deadline: .now() + Self.statsFlushIdleDelay, execute: item)
    }

    private func flushPendingStatsLocked() {
        idleFlush?.cancel()
        idleFlush = nil
        let pending = pendingStats
        guard pending.total > 0 else { return }
        pendingStats = PendingStats()
        addToStoredStats(total: pending.total, sh: pending.sh, ch: pending.ch, o: pending.o, g: pending.g, ng: pending.ng)
    }

    private func addToStoredStats(total: Int, sh: Int, ch: Int, o: Int, g: Int, ng: Int) {
        func bump(_ key: String, by amount: Int) {
            guard amount > 0 else { return }
            userDefaults.set(userDefaults.integer(forKey: key) + amount, forKey: key)
        }
        bump(Self.keyStatsTotal, by: total)
        bump(Self.keyStatsSh, by: sh)
        bump(Self.keyStatsCh, by: ch)
        bump(Self.keyStatsO, by: o)
        bump(Self.keyStatsG, by: g)
        bump(Self.keyStatsNg, by: ng)
    }

    public func recordBatchConversions(totalAdded: Int, sh: Int = 0, ch: Int = 0, o: Int = 0, g: Int = 0, ng: Int = 0) {
        addToStoredStats(total: totalAdded, sh: sh, ch: ch, o: o, g: g, ng: ng)
        reloadStats()
    }

    public func clearStats() {
        userDefaults.removeObject(forKey: Self.keyStatsTotal)
        userDefaults.removeObject(forKey: Self.keyStatsSh)
        userDefaults.removeObject(forKey: Self.keyStatsCh)
        userDefaults.removeObject(forKey: Self.keyStatsO)
        userDefaults.removeObject(forKey: Self.keyStatsG)
        userDefaults.removeObject(forKey: Self.keyStatsNg)
        reloadStats()
    }
}
