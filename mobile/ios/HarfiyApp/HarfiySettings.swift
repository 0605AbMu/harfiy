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
    public static let keyKeyboardLastOpened = "keyboard_last_opened_timestamp"

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

    /// Reload user preferences from shared UserDefaults (e.g. when keyboard appears)
    public func reloadSettings() {
        self.instantUndo = userDefaults.bool(forKey: Self.keyInstantUndo)
        self.enableNg = userDefaults.bool(forKey: Self.keyEnableNg)
        self.smartApostrophe = userDefaults.bool(forKey: Self.keySmartApostrophe)
        self.hapticFeedback = userDefaults.bool(forKey: Self.keyHapticFeedback)
        self.audioFeedback = userDefaults.bool(forKey: Self.keyAudioFeedback)
        self.hasCompletedWizard = userDefaults.bool(forKey: Self.keyHasCompletedWizard)
    }

    @Published public var instantUndo: Bool {
        didSet { userDefaults.set(instantUndo, forKey: Self.keyInstantUndo) }
    }

    @Published public var enableNg: Bool {
        didSet { userDefaults.set(enableNg, forKey: Self.keyEnableNg) }
    }

    @Published public var smartApostrophe: Bool {
        didSet { userDefaults.set(smartApostrophe, forKey: Self.keySmartApostrophe) }
    }

    @Published public var hapticFeedback: Bool {
        didSet { userDefaults.set(hapticFeedback, forKey: Self.keyHapticFeedback) }
    }

    @Published public var audioFeedback: Bool {
        didSet { userDefaults.set(audioFeedback, forKey: Self.keyAudioFeedback) }
    }

    @Published public var hasCompletedWizard: Bool {
        didSet { userDefaults.set(hasCompletedWizard, forKey: Self.keyHasCompletedWizard) }
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

    public func recordConversion(character: String) {
        // Run stats writing asynchronously off the main thread so typing is never blocked
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self = self else { return }
            let total = self.userDefaults.integer(forKey: Self.keyStatsTotal) + 1
            self.userDefaults.set(total, forKey: Self.keyStatsTotal)

            let lower = character.lowercased()
            if lower.contains("ş") {
                let val = self.userDefaults.integer(forKey: Self.keyStatsSh) + 1
                self.userDefaults.set(val, forKey: Self.keyStatsSh)
            } else if lower.contains("ç") {
                let val = self.userDefaults.integer(forKey: Self.keyStatsCh) + 1
                self.userDefaults.set(val, forKey: Self.keyStatsCh)
            } else if lower.contains("ö") || lower.contains("oʻ") || lower.contains("o'") {
                let val = self.userDefaults.integer(forKey: Self.keyStatsO) + 1
                self.userDefaults.set(val, forKey: Self.keyStatsO)
            } else if lower.contains("ğ") || lower.contains("gʻ") || lower.contains("g'") {
                let val = self.userDefaults.integer(forKey: Self.keyStatsG) + 1
                self.userDefaults.set(val, forKey: Self.keyStatsG)
            } else if lower.contains("ñ") {
                let val = self.userDefaults.integer(forKey: Self.keyStatsNg) + 1
                self.userDefaults.set(val, forKey: Self.keyStatsNg)
            }
        }
    }

    public func recordBatchConversions(totalAdded: Int, sh: Int = 0, ch: Int = 0, o: Int = 0, g: Int = 0, ng: Int = 0) {
        let currentTotal = userDefaults.integer(forKey: Self.keyStatsTotal) + totalAdded
        userDefaults.set(currentTotal, forKey: Self.keyStatsTotal)

        if sh > 0 {
            userDefaults.set(userDefaults.integer(forKey: Self.keyStatsSh) + sh, forKey: Self.keyStatsSh)
        }
        if ch > 0 {
            userDefaults.set(userDefaults.integer(forKey: Self.keyStatsCh) + ch, forKey: Self.keyStatsCh)
        }
        if o > 0 {
            userDefaults.set(userDefaults.integer(forKey: Self.keyStatsO) + o, forKey: Self.keyStatsO)
        }
        if g > 0 {
            userDefaults.set(userDefaults.integer(forKey: Self.keyStatsG) + g, forKey: Self.keyStatsG)
        }
        if ng > 0 {
            userDefaults.set(userDefaults.integer(forKey: Self.keyStatsNg) + ng, forKey: Self.keyStatsNg)
        }
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
