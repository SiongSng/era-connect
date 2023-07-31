import 'package:flutter/foundation.dart';

import 'ffi.dart' show api;

export 'gen/bridge_definitions.dart' show UILayout;
export 'config_api.dart';

void initializeAPIs() async {
  final loggerStream = api.setupLogger();
  loggerStream.listen((log) {
    if (kDebugMode) {
      final message =
          '[${DateTime.fromMillisecondsSinceEpoch(log.timestamp)}] [${log.level}] ${log.message}';
      print(message);
    }
  });
}
