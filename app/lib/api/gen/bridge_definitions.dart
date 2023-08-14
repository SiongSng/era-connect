// AUTO GENERATED FILE, DO NOT EDIT.
// Generated by `flutter_rust_bridge`@ 1.80.1.
// ignore_for_file: non_constant_identifier_names, unused_element, duplicate_ignore, directives_ordering, curly_braces_in_flow_control_structures, unnecessary_lambdas, slash_for_doc_comments, prefer_const_literals_to_create_immutables, implicit_dynamic_list_literal, duplicate_import, unused_import, unnecessary_import, prefer_single_quotes, prefer_const_constructors, use_super_parameters, always_use_package_imports, annotate_overrides, invalid_use_of_protected_member, constant_identifier_names, invalid_use_of_internal_member, prefer_is_empty, unnecessary_const

import 'dart:convert';
import 'dart:async';
import 'package:meta/meta.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:uuid/uuid.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;

part 'bridge_definitions.freezed.dart';

abstract class Native {
  Future<void> setupLogger({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kSetupLoggerConstMeta;

  Stream<Progress> downloadVanilla({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kDownloadVanillaConstMeta;

  Stream<Progress> launchVanilla({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kLaunchVanillaConstMeta;

  Stream<Progress> launchForge({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kLaunchForgeConstMeta;

  Stream<Progress> launchQuilt({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kLaunchQuiltConstMeta;

  Future<DownloadState> fetchState({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kFetchStateConstMeta;

  Future<void> writeState({required DownloadState s, dynamic hint});

  FlutterRustBridgeTaskConstMeta get kWriteStateConstMeta;

  Value getUiLayoutConfig({required Key key, dynamic hint});

  FlutterRustBridgeTaskConstMeta get kGetUiLayoutConfigConstMeta;

  Future<void> setUiLayoutConfig({required Value value, dynamic hint});

  FlutterRustBridgeTaskConstMeta get kSetUiLayoutConfigConstMeta;

  Stream<LoginFlowEvent> minecraftLoginFlow({dynamic hint});

  FlutterRustBridgeTaskConstMeta get kMinecraftLoginFlowConstMeta;
}

class AccountToken {
  final String token;
  final int expiresAt;

  const AccountToken({
    required this.token,
    required this.expiresAt,
  });
}

enum DownloadState {
  Downloading,
  Paused,
  Stopped,
}

enum Key {
  CompletedSetup,
}

class LoginFlowDeviceCode {
  final String verificationUri;
  final String userCode;

  const LoginFlowDeviceCode({
    required this.verificationUri,
    required this.userCode,
  });
}

@freezed
sealed class LoginFlowErrors with _$LoginFlowErrors {
  const factory LoginFlowErrors.xstsError(
    XstsTokenErrorType field0,
  ) = LoginFlowErrors_XstsError;
  const factory LoginFlowErrors.gameNotOwned() = LoginFlowErrors_GameNotOwned;
  const factory LoginFlowErrors.unknownError() = LoginFlowErrors_UnknownError;
}

@freezed
sealed class LoginFlowEvent with _$LoginFlowEvent {
  const factory LoginFlowEvent.stage(
    LoginFlowStage field0,
  ) = LoginFlowEvent_Stage;
  const factory LoginFlowEvent.deviceCode(
    LoginFlowDeviceCode field0,
  ) = LoginFlowEvent_DeviceCode;
  const factory LoginFlowEvent.error(
    LoginFlowErrors field0,
  ) = LoginFlowEvent_Error;
  const factory LoginFlowEvent.success(
    MinecraftAccount field0,
  ) = LoginFlowEvent_Success;
}

enum LoginFlowStage {
  FetchingDeviceCode,
  WaitingForUser,
  AuthenticatingXboxLive,
  FetchingXstsToken,
  FetchingMinecraftToken,
  CheckingGameOwnership,
  GettingProfile,
}

class MinecraftAccount {
  final String username;
  final UuidValue uuid;
  final AccountToken accessToken;
  final AccountToken refreshToken;
  final List<MinecraftSkin> skins;
  final List<MinecraftCape> capes;

  const MinecraftAccount({
    required this.username,
    required this.uuid,
    required this.accessToken,
    required this.refreshToken,
    required this.skins,
    required this.capes,
  });
}

class MinecraftCape {
  final UuidValue id;
  final String state;
  final String url;
  final String alias;

  const MinecraftCape({
    required this.id,
    required this.state,
    required this.url,
    required this.alias,
  });
}

class MinecraftSkin {
  final UuidValue id;
  final String state;
  final String url;
  final MinecraftSkinVariant variant;

  const MinecraftSkin({
    required this.id,
    required this.state,
    required this.url,
    required this.variant,
  });
}

enum MinecraftSkinVariant {
  Classic,
  Slim,
}

class Progress {
  final double speed;
  final double percentages;
  final double currentSize;
  final double totalSize;

  const Progress({
    required this.speed,
    required this.percentages,
    required this.currentSize,
    required this.totalSize,
  });
}

@freezed
sealed class Value with _$Value {
  const factory Value.completedSetup(
    bool field0,
  ) = Value_CompletedSetup;
}

/// Reference: https://wiki.vg/Microsoft_Authentication_Scheme
enum XstsTokenErrorType {
  /// The account doesn't have an Xbox account. Once they sign up for one (or login through minecraft.net to create one) then they can proceed with the login. This shouldn't happen with accounts that have purchased Minecraft with a Microsoft account, as they would've already gone through that Xbox signup process.
  DoesNotHaveXboxAccount,

  /// The account is from a country where Xbox Live is not available/banned.
  CountryNotAvailable,

  /// The account needs adult verification on Xbox page. (South Korea)
  NeedsAdultVerificationKR1,

  /// The account needs adult verification on Xbox page. (South Korea)
  NeedsAdultVerificationKR2,

  /// The account is a child (under 18) and cannot proceed unless the account is added to a Family by an adult. This only seems to occur when using a custom Microsoft Azure application. When using the Minecraft launchers client id, this doesn't trigger.
  ChildAccount,
}
