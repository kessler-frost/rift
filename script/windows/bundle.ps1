#!/usr/bin/env powershell
#
# Bundle the application for release.

Param (
    # Build dev bundles by default.
    [Switch]$DEBUG_BUILD = $False,

    [Alias('check-only')]
    [Switch]$CHECK_ONLY,

    [ValidateSet('local', 'dev', 'preview', 'stable', 'oss')]
    [String]$CHANNEL = 'dev',

    [Alias('release-tag')]
    [String]$RELEASE_TAG = '',
    [String]$FEATURES = 'release_bundle,crash_reporting,gui',

    # Builds only the Rift binary, skips the installer.
    [Switch]$SKIP_BUILD_INSTALLER = $False,
    # Builds only the installer, skips the Rift binary. Use this if the Rift
    # binary has already been built.
    [Switch]$SKIP_BUILD_BINARY = $False,

    [ValidateSet('x64', 'arm64')]
    [String]$ARCH = '',

    # A signtool command for Inno Setup to sign the setup engine and uninstaller.
    # Uses $f as the file placeholder, e.g.:
    #   'signtool.exe sign /fd SHA256 ... $f'
    # When empty, the installer is built without signing.
    [Alias('sign-tool-cmd')]
    [String]$SIGN_TOOL_CMD = ''
)

if ($RELEASE_TAG) {
    $env:GIT_RELEASE_TAG = $RELEASE_TAG
}

# Use provided ARCH parameter if set, otherwise detect from system
if (-not $ARCH) {
    if ($env:PROCESSOR_ARCHITECTURE -eq 'AMD64') {
        $ARCH = 'x64'
    } elseif ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
        $ARCH = 'arm64'
    } else {
        throw "Unsupported processor architecture: $env:PROCESSOR_ARCHITECTURE"
    }
}

if ($ARCH -eq 'arm64') {
    $FILE_ENDING = 'Setup-arm64'
    $PLATFORM_TARGET = 'aarch64-pc-windows-msvc'
} else {
    # If x64, then we just use the filename "RiftSetup.exe" for example
    $FILE_ENDING = 'Setup'
    $PLATFORM_TARGET = 'x86_64-pc-windows-msvc'
}

$ErrorActionPreference = 'Stop'

$WORKSPACE_ROOT_DIR = $(Get-Location).Path
$CARGO_TARGET_DIR = $WORKSPACE_ROOT_DIR + '\target'
$WINDOWS_INSTALLER_DIR = $WORKSPACE_ROOT_DIR + '\script\windows'

if ($DEBUG_BUILD) {
    $CARGO_PROFILE = 'dev'
} elseif (("$CHANNEL" -eq 'local') -or ("$CHANNEL" -eq 'dev')) {
    # For dev bundles, we want to enable debug assertions to
    # catch violations that would otherwise silently pass in
    # a normal release build (e.g. in stable).
    $CARGO_PROFILE = 'rltoda'
} else {
    $CARGO_PROFILE = 'rlto'
}

if ($CARGO_PROFILE -eq 'dev') {
    $CARGO_TARGET_OUTPUT_DIR = "$CARGO_TARGET_DIR" + '\' + $PLATFORM_TARGET + '\debug'
} else {
    $CARGO_TARGET_OUTPUT_DIR = "$CARGO_TARGET_DIR" + '\' + $PLATFORM_TARGET + '\' + "$CARGO_PROFILE"
}
$BUNDLE_ID = "dev.rift.$app_name"

# Update parameters based on the target release channel.
#
# APP_NAME here must match the value used in Rust as the
# application name; see app/src/channel.rs.
#
# RIFT_BIN is the name of the binary produced by cargo;
# BINARY_NAME is the desired name of the binary in the final package.
if ("$CHANNEL" -eq 'local') {
    $RIFT_BIN = 'rift'
    $BINARY_NAME = 'rift.exe'
    $APP_NAME = 'RiftLocal'
} elseif ("$CHANNEL" -eq 'dev') {
    $RIFT_BIN = 'dev'
    $BINARY_NAME = 'dev.exe'
    $APP_NAME = 'RiftDev'
    $FEATURES = "$FEATURES,agent_mode_debug"
} elseif ("$CHANNEL" -eq 'preview') {
    $RIFT_BIN = 'preview'
    $BINARY_NAME = 'preview.exe'
    $APP_NAME = 'RiftPreview'
    $FEATURES = "$FEATURES,preview_channel"
} elseif ("$CHANNEL" -eq 'stable') {
    $RIFT_BIN = 'stable'
    $BINARY_NAME = 'rift.exe'
    $APP_NAME = 'Rift'
} elseif ("$CHANNEL" -eq 'oss') {
    $RIFT_BIN = 'rift-oss'
    $BINARY_NAME = 'rift-oss.exe'
    $APP_NAME = 'Rift'
    # The OSS channel does not ship Sentry, so drop the crash_reporting feature
    # (which would otherwise pull in the Sentry SDK as a dependency).
    $FEATURES = 'release_bundle,gui'
}

if (("$CHANNEL" -eq 'local') -or ("$CHANNEL" -eq 'dev')) {
    $FEATURES = "$FEATURES,nld_classifier_v3,nld_heuristic_v2"
} elseif ("$CHANNEL" -eq 'preview') {
    $FEATURES = "$FEATURES,nld_classifier_v2,nld_heuristic_v2"
} else {
    $FEATURES = "$FEATURES,nld_classifier_v1,nld_heuristic_v1"
}

$BINARY_PATH = "$CARGO_TARGET_OUTPUT_DIR\$BINARY_NAME"
$BUNDLE_ID = "dev.rift.$APP_NAME"
$INSTALLER_OUTPUT_DIR = "$WINDOWS_INSTALLER_DIR\Output"
$INSTALLER_NAME = "$($APP_NAME)$($FILE_ENDING)"
$INSTALLER_PATH = "$($INSTALLER_OUTPUT_DIR)\$($INSTALLER_NAME).exe"
$PDB_PATH = "$CARGO_TARGET_OUTPUT_DIR\$RIFT_BIN.pdb"

# The CARGO_FULL_PROFILE environment variable is read by the `cargo` build
# script (`app/build.rs`) to determine where to place `conpty.dll`.
if ($DEBUG_BUILD) {
    $env:CARGO_FULL_PROFILE = 'debug'
} else {
    $env:CARGO_FULL_PROFILE = $CARGO_PROFILE
}

# If we only want to check that compilation will succeed, perform the checks
# then exit.  We use this script to invoke `cargo check` to ensure that we are
# using the same feature flags and profile that we would be using in production.
if ($CHECK_ONLY) {
    cargo check -p rift --profile "$CARGO_PROFILE" --bin "$RIFT_BIN" --features "$FEATURES" --target $PLATFORM_TARGET
    if (-Not $?) {
        Write-Error "Failed to verify Rift $RIFT_BIN compilation with profile $CARGO_PROFILE"
        exit 1
    }
    exit 0
}

if (-Not $SKIP_BUILD_BINARY) {
    Write-Output "Building Rift for channel $CHANNEL and bundle id $BUNDLE_ID"
    $env:CARGO_BIN_NAME = $CHANNEL
    $env:RIFT_APP_NAME = $APP_NAME
    cargo build -p rift --profile "$CARGO_PROFILE" --bin "$RIFT_BIN" --features "$FEATURES" --target $PLATFORM_TARGET
    if (-Not $?) {
        Write-Error "Failed to build Rift $RIFT_BIN binary with profile $CARGO_PROFILE"
        exit 1
    }

    # If we desire an executable name different from the cargo bin, rename it.
    if ("$RIFT_BIN.exe" -ne $BINARY_NAME) {
        $binarySource = "$CARGO_TARGET_OUTPUT_DIR\$RIFT_BIN.exe"
        Write-Output "Renaming executable $RIFT_BIN.exe to $BINARY_NAME"
        Move-Item -Path "$binarySource" -Destination "$BINARY_PATH" -Force
    }
}

if ($SKIP_BUILD_INSTALLER) {
    # If this is being run within a GitHub action, set an output variable with the
    # location of the binary so it can be referenced by subsequent actions.
    if ($env:GITHUB_ACTIONS -eq 'true') {
        Write-Output '::echo::on'
        "target_profile_dir=$CARGO_TARGET_OUTPUT_DIR" >> "$env:GITHUB_OUTPUT"
        "binary_path=$BINARY_PATH" >> "$env:GITHUB_OUTPUT"
        Write-Output '::echo::off'
    }
    exit 0
}

Write-Output "Built for $ARCH with executable at $BINARY_PATH"

# Prepare bundled resources
$BUNDLED_RESOURCES_DIR = "$CARGO_TARGET_OUTPUT_DIR\resources"
Write-Output "Preparing bundled resources..."
& "$WINDOWS_INSTALLER_DIR\prepare_bundled_resources.ps1" -DestinationDir "$BUNDLED_RESOURCES_DIR" -Channel "$CHANNEL" -CargoProfile "$CARGO_PROFILE"
if (-Not $?) {
    Write-Error "Failed to prepare bundled resources"
    exit 1
}

Write-Output 'Building Rift installer'
$ISCC_ARGS = @(
    "$WINDOWS_INSTALLER_DIR\windows-installer.iss",
    "/DReleaseChannel=$CHANNEL",
    "/DMyAppExeName=$BINARY_NAME",
    "/DTargetProfileDir=$CARGO_TARGET_OUTPUT_DIR",
    "/DMyAppName=$APP_NAME",
    "/DMyAppVersion=$env:GIT_RELEASE_TAG",
    "/DArch=$ARCH",
    "/DOutputName=$INSTALLER_NAME"
)
# Also accept the sign tool command via env var
if (-not $SIGN_TOOL_CMD -and $env:SIGN_TOOL_CMD) {
    $SIGN_TOOL_CMD = $env:SIGN_TOOL_CMD
}
if ($SIGN_TOOL_CMD) {
    $ISCC_ARGS += '/DSIGN_TOOL=1'
    $ISCC_ARGS += "/Scodesign=$SIGN_TOOL_CMD"
}
& ISCC @ISCC_ARGS
if (-Not $?) {
    Write-Error "Failed to build $APP_NAME installer"
    exit 1
}

# If this is being run within a GitHub action, set an output variable with the
# location of the installer so it can be referenced by subsequent actions.
if ($env:GITHUB_ACTIONS -eq 'true') {
    Write-Output '::echo::on'
    $INSTALLER_PATH = $INSTALLER_PATH -replace '\\', '/'
    "installer_path=$INSTALLER_PATH" >> "$env:GITHUB_OUTPUT"
    "pdb_file_path=$PDB_PATH" >> "$env:GITHUB_OUTPUT"
    Write-Output '::echo::off'
}
