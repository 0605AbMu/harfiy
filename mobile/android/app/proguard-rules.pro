# Harfiy Proguard / R8 Configuration for Google Play Release

# Preserve all native methods and JNI bindings
-keepclasseswithmembernames class * {
    native <methods>;
}

# Preserve Accessibility Service and Tile Service entrypoints
-keep public class uz.harfiy.keyboard.HarfiyAccessibilityService {
    public *;
}
-keep public class uz.harfiy.keyboard.HarfiyTileService {
    public *;
}
-keep public class uz.harfiy.keyboard.MainActivity {
    public *;
}
-keep public class uz.harfiy.keyboard.HarfiyUpdateReceiver {
    public *;
}

# Preserve annotations and line numbers for crash debugging
-keepattributes *Annotation*,Signature,InnerClasses,EnclosingMethod,SourceFile,LineNumberTable
