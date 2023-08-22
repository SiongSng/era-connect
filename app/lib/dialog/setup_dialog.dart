import 'package:era_connect/api/lib.dart';
import 'package:era_connect_ui/components/dialog/step_dialog.dart';
import 'package:era_connect_i18n/era_connect_i18n.dart';
import 'package:era_connect_ui/era_connect_ui.dart';
import 'package:flutter/material.dart';

import 'login_account_dialog.dart';

class SetupDialog extends StatelessWidget {
  const SetupDialog({super.key});

  @override
  Widget build(BuildContext context) {
    return StepDialog(
      steps: [
        StepData(
          stepDescription: '匯入或新建設定',
          title: context.i18n['dialog.setup.welcome'],
          description: context.i18n['dialog.setup.01.description'],
          hasBrandText: true,
          contentPages: const [_ImportProfiles()],
        ),
        StepData(
            stepDescription: '登入帳號',
            title: context.i18n['dialog.setup.welcome'],
            description: context.i18n['dialog.setup.01.description'],
            contentPages: const [_LoginAccount()]),
        StepData(
            stepDescription: '建立第一個收藏',
            title: context.i18n['dialog.setup.welcome'],
            description: 'Test Description',
            contentPages: [
              const Text('Test 3 PAGE 1'),
              const Text('Test 3 PAGE 2'),
              const Text('Test 3 PAGE 3')
            ]),
        StepData(
            stepDescription: '為您的收藏加入內容',
            title: context.i18n['dialog.setup.welcome'],
            description: 'Test Description',
            contentPages: [const Text('Test 4')]),
        StepData(
          stepDescription: '大功告成！',
          title: '大功告成！',
          description: 'Test Description',
          contentPages: [const SizedBox.shrink()],
          onEvent: (event) {
            if (event == StepEvent.done) {
              storageApi.uiLayout.completedSetup = true;
            }

            return true;
          },
        )
      ],
    );
  }
}

class _ImportProfiles extends StatelessWidget {
  const _ImportProfiles();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          context.i18n['dialog.setup.01.content.title'],
          style: TextStyle(
            color: context.theme.textColor,
            fontSize: 40,
            fontWeight: FontWeight.w700,
          ),
        ),
        Text(
          context.i18n['dialog.setup.01.content.description'],
          style:
              TextStyle(color: context.theme.tertiaryTextColor, fontSize: 15),
        ),
        const SizedBox(height: 25),
        Expanded(
          child: DialogRectangleTab(
            title: '選擇方式',
            tabs: [
              TabItem(
                title: context.i18n['dialog.setup.01.content.tabs.empty.title'],
                icon: 'deployed_code',
              ),
              TabItem(
                  title:
                      context.i18n['dialog.setup.01.content.tabs.import.title'],
                  icon: 'deployed_code_update',
                  content: const DialogContentBox(
                    title: '匯入平台',
                    content: SizedBox.shrink(),
                  )),
            ],
          ),
        ),
      ],
    );
  }
}

class _LoginAccount extends StatelessWidget {
  const _LoginAccount();

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          '登入帳號',
          style: TextStyle(
            color: context.theme.textColor,
            fontSize: 40,
            fontWeight: FontWeight.w700,
          ),
        ),
        Text(
          '登入帳號後即可開始享受遊戲的樂趣！',
          style:
              TextStyle(color: context.theme.tertiaryTextColor, fontSize: 15),
        ),
        const SizedBox(height: 25),
        Expanded(
          child: DialogRectangleTab(
            title: '選擇方式',
            tabs: [
              TabItem(
                title: '只登入主要帳號',
                icon: 'person',
                content: _buildSingleAccount(context),
              ),
              const TabItem(
                  title: '登入多個帳號',
                  icon: 'group',
                  content: DialogContentBox(
                    title: '多個帳號',
                    content: SizedBox.shrink(),
                  )),
            ],
          ),
        ),
      ],
    );
  }

  Column _buildSingleAccount(BuildContext context) {
    final accounts = storageApi.accountStorage.accounts;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: DialogContentBox(
            title: '登入動作',
            content: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: accounts.isEmpty
                      ? _buildLoginButton(context)
                      : _buildAccountTile(context, accounts.first),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: 15),
        Text(
          '此動作將僅登入一個主要帳號，稍後你可以在帳號管理頁面中新增更多帳號。',
          style:
              TextStyle(color: context.theme.tertiaryTextColor, fontSize: 13),
        )
      ],
    );
  }

  Widget _buildAccountTile(BuildContext context, MinecraftAccount account) {
    return Material(
      child: ListTile(
        leading: ClipRRect(
          borderRadius: BorderRadius.circular(10),
          child: account.skins.first.renderHead(size: 50),
        ),
        title: Text(account.username),
        subtitle: Text(
          'Minecraft 帳號',
          style: TextStyle(color: context.theme.tertiaryTextColor),
        ),
        trailing: EraDialogButton.textSecondary(
          text: '重新登入',
          onPressed: () async {
            await storageApi.accountStorage.removeAccount(account.uuid);

            if (!context.mounted) return;
            _showLoginAccountDialog(context);
          },
        ),
        tileColor: context.theme.deepBackgroundColor,
        contentPadding:
            const EdgeInsets.symmetric(vertical: 10, horizontal: 15),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(15),
        ),
      ),
    );
  }

  Widget _buildLoginButton(BuildContext context) {
    return TextButton.icon(
      icon: const Icon(Icons.person_add_alt_rounded),
      label: Text('立即登入', style: TextStyle(color: context.theme.textColor)),
      style: TextButton.styleFrom(
        backgroundColor: context.theme.deepBackgroundColor,
        iconColor: context.theme.textColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(15),
        ),
        padding: const EdgeInsets.symmetric(horizontal: 15, vertical: 22),
      ),
      onPressed: () => _showLoginAccountDialog(context),
    );
  }

  Future<void> _showLoginAccountDialog(BuildContext context) {
    return showEraDialog(
      context: context,
      barrierDismissible: true,
      dialog: const LoginAccountDialog(),
    );
  }
}
