// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.21.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../../frb_generated.dart';
import '../backend_exclusive/storage/storage_loader.dart';
import '../backend_exclusive/vanilla/launcher.dart';
import '../backend_exclusive/vanilla/version.dart';
import 'authentication/account.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

// Rust type: RustOpaqueNom<flutter_rust_bridge::for_generated::rust_async::RwLock<Collection>>
@sealed
class Collection extends RustOpaque {
  Collection.dcoDecode(List<dynamic> wire)
      : super.dcoDecode(wire, _kStaticData);

  Collection.sseDecode(int ptr, int externalSizeOnNative)
      : super.sseDecode(ptr, externalSizeOnNative, _kStaticData);

  static final _kStaticData = RustArcStaticData(
    rustArcIncrementStrongCount:
        RustLib.instance.api.rust_arc_increment_strong_count_Collection,
    rustArcDecrementStrongCount:
        RustLib.instance.api.rust_arc_decrement_strong_count_Collection,
    rustArcDecrementStrongCountPtr:
        RustLib.instance.api.rust_arc_decrement_strong_count_CollectionPtr,
  );

  /// Creates a collection and return a collection with its loader attached
  static Future<Collection> create(
          {required String displayName,
          required VersionMetadata versionMetadata,
          ModLoader? modLoader,
          AdvancedOptions? advancedOptions,
          dynamic hint}) =>
      RustLib.instance.api.collectionCreate(
          displayName: displayName,
          versionMetadata: versionMetadata,
          modLoader: modLoader,
          advancedOptions: advancedOptions,
          hint: hint);

  Future<PathBuf> gameDirectory({dynamic hint}) =>
      RustLib.instance.api.collectionGameDirectory(
        that: this,
      );

  static Future<PathBuf> getBasePath({dynamic hint}) =>
      RustLib.instance.api.collectionGetBasePath(hint: hint);

  Future<CollectionId> getCollectionId({dynamic hint}) =>
      RustLib.instance.api.collectionGetCollectionId(
        that: this,
      );

  /// SIDE-EFFECT: put `launch_args` into Struct
  Future<void> launchGame({dynamic hint}) =>
      RustLib.instance.api.collectionLaunchGame(
        that: this,
      );

  Future<void> save({dynamic hint}) => RustLib.instance.api.collectionSave(
        that: this,
      );

  static Future<List<StorageLoader>> scan({dynamic hint}) =>
      RustLib.instance.api.collectionScan(hint: hint);

  /// Downloads game(also verifies)
  Future<LaunchArgs> verifyAndDownloadGame({dynamic hint}) =>
      RustLib.instance.api.collectionVerifyAndDownloadGame(
        that: this,
      );
}

class AdvancedOptions {
  final int? jvmMaxMemory;

  const AdvancedOptions({
    this.jvmMaxMemory,
  });

  @override
  int get hashCode => jvmMaxMemory.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AdvancedOptions &&
          runtimeType == other.runtimeType &&
          jvmMaxMemory == other.jvmMaxMemory;
}

class CollectionId {
  final String field0;

  const CollectionId({
    required this.field0,
  });

  @override
  int get hashCode => field0.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CollectionId &&
          runtimeType == other.runtimeType &&
          field0 == other.field0;
}

class ModLoader {
  final ModLoaderType modLoaderType;
  final String? version;

  const ModLoader({
    required this.modLoaderType,
    this.version,
  });

  @override
  int get hashCode => modLoaderType.hashCode ^ version.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ModLoader &&
          runtimeType == other.runtimeType &&
          modLoaderType == other.modLoaderType &&
          version == other.version;
}

enum ModLoaderType {
  forge,
  neoForge,
  fabric,
  quilt,
}
