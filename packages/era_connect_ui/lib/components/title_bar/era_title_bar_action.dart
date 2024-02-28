import 'package:era_connect_ui/components/lib.dart';
import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

class EraTitleBarAction extends StatelessWidget {
  final Widget icon;
  final VoidCallback onPressed;
  final Color? hoverColor;

  const EraTitleBarAction._(
      {required this.icon, required this.onPressed, this.hoverColor});

  @override
  Widget build(BuildContext context) {
    return IconButton(
      icon: IconTheme(data: const IconThemeData(size: 20), child: icon),
      hoverColor: hoverColor,
      style: IconButton.styleFrom(
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(0),
        ),
      ),
      onPressed: onPressed,
    );
  }

  factory EraTitleBarAction.minimize() {
    return EraTitleBarAction._(
      icon: EraIcon.material(Icons.drag_handle),
      onPressed: () {
        windowManager.minimize();
      },
    );
  }

  factory EraTitleBarAction.maximize() {
    return EraTitleBarAction._(
      icon: EraIcon.assets('thumbnail_bar'),
      onPressed: () async {
        final isMaximized = await windowManager.isMaximized();

        if (isMaximized) {
          await windowManager.unmaximize();
        } else {
          await windowManager.maximize();
        }
      },
    );
  }

  factory EraTitleBarAction.close() {
    return EraTitleBarAction._(
      icon: EraIcon.assets('close'),
      hoverColor: const Color(0xffff0000),
      onPressed: () {
        windowManager.close();
      },
    );
  }
}
