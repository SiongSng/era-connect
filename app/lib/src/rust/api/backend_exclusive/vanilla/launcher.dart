// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.21.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../../../frb_generated.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

class LaunchArgs {
  final List<String> jvmArgs;
  final String mainClass;
  final List<String> gameArgs;

  const LaunchArgs({
    required this.jvmArgs,
    required this.mainClass,
    required this.gameArgs,
  });

  @override
  int get hashCode => jvmArgs.hashCode ^ mainClass.hashCode ^ gameArgs.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LaunchArgs &&
          runtimeType == other.runtimeType &&
          jvmArgs == other.jvmArgs &&
          mainClass == other.mainClass &&
          gameArgs == other.gameArgs;
}
