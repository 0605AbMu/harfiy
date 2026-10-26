//
//  HarfiyKeyboardStyleService.swift
//  HarfiyKeyboard
//
//  Custom KeyboardStyleService for Harfiy Keyboard.
//  Configures native-like callout proportions and sizes to prevent callout bubbles
//  from clipping at the top edge of the iOS keyboard extension window (UIRemoteKeyboardWindow).
//

import Foundation
import SwiftUI
import KeyboardKit

/// Custom keyboard style service providing perfected geometry and aesthetics for Harfiy
public class HarfiyKeyboardStyleService: KeyboardStyle.StandardService {

    public override init(keyboardContext: KeyboardContext) {
        super.init(keyboardContext: keyboardContext)
    }

    /// Style applied to the character preview popup bubble when a key is pressed.
    public override var inputCalloutStyle: Callouts.InputCalloutStyle {
        var style = super.inputCalloutStyle
        // Native iOS callout bubble height is ~44pt.
        // Combined with our 44pt toolbar above the top row, the callout stays
        // safely within bounds (y >= 0) and will never be clipped at the top.
        style.calloutSize = CGSize(width: 0, height: 44)
        style.calloutPadding = 2
        style.font = .init(.title2, .medium)
        return style
    }

    /// Style applied when a key is long-pressed to reveal secondary actions.
    public override var actionCalloutStyle: Callouts.ActionCalloutStyle {
        var style = super.actionCalloutStyle
        style.maxButtonSize = CGSize(width: 44, height: 44)
        style.font = .init(.title3, .medium)
        style.verticalOffset = 0
        return style
    }

    /// Shared callout styling for background, corner radii, and curves.
    public override var calloutStyle: Callouts.CalloutStyle {
        var style = super.calloutStyle
        style.cornerRadius = 10
        style.buttonCornerRadius = 5
        style.curveSize = CGSize(width: 6, height: 12)
        return style
    }
}
