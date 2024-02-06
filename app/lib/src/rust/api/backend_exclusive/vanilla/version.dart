// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.23.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../../../frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;
part 'version.freezed.dart';

@freezed
class VersionMetadata with _$VersionMetadata {
  const factory VersionMetadata({
    required String id,
    required VersionType versionType,
    required String url,
    required DateTime uploadedTime,
    required DateTime releaseTime,
    required String sha1,
    required int complianceLevel,
  }) = _VersionMetadata;
}

enum VersionType {
  release,
  snapshot,
  oldBeta,
  oldAlpha,
}
