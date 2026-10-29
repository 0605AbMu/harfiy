//
//  HarfiyPerf.swift
//  HarfiyKeyboard
//
//  Signposts for profiling the keyboard in Instruments (Points of Interest / os_signpost).
//  They cost next to nothing while no profiler is attached, so they stay in release builds.
//

import os

enum HarfiyPerf {
    static let signposter = OSSignposter(subsystem: "uz.harfiy.keyboard", category: "PointsOfInterest")

    /// Records how much memory the extension may still allocate before the system terminates it.
    static func emitAvailableMemory() {
        let megabytes = os_proc_available_memory() / 1_048_576
        signposter.emitEvent("availableMemoryMB", "\(megabytes)")
    }
}
