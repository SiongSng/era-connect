// This file is automatically generated, so please do not edit it.
// Generated by `flutter_rust_bridge`@ 2.0.0-dev.23.

// ignore_for_file: invalid_use_of_internal_member, unused_import, unnecessary_import

import '../../../frb_generated.dart';
import '../../shared_resources/authentication/account.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';
import 'package:freezed_annotation/freezed_annotation.dart' hide protected;
import 'package:uuid/uuid.dart';
part 'account_storage.freezed.dart';

enum AccountStorageKey {
  accounts,
  mainAccount,
}

@freezed
sealed class AccountStorageValue with _$AccountStorageValue {
  const factory AccountStorageValue.accounts(
    List<MinecraftAccount> field0,
  ) = AccountStorageValue_Accounts;
  const factory AccountStorageValue.mainAccount([
    UuidValue? field0,
  ]) = AccountStorageValue_MainAccount;
}
