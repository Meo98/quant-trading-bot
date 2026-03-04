import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:matrix_quant_app/src/rust/frb_generated.dart';
import 'package:matrix_quant_app/src/rust/api/simple.dart';
import 'package:local_auth/local_auth.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_background_service/flutter_background_service.dart';
import 'dart:async';
import 'dart:developer';
import 'dart:ui';
import 'dart:convert';

// =============================================================================
// CONSTANTS & THEME
// =============================================================================

class AppColors {
  static const background = Color(0xFF0F172A);
  static const surface = Color(0xFF1E293B);
  static const accent = Colors.tealAccent;
  static const profit = Color(0xFF22C55E);
  static const loss = Color(0xFFEF4444);
  static const warning = Color(0xFFF59E0B);
}

// =============================================================================
// STRATEGY PRESETS
// =============================================================================

enum StrategyType { conservative, moderate, aggressive, custom }

class StrategyPreset {
  final String name;
  final String description;
  final IconData icon;
  final Color color;
  final double minPct24h;
  final double minPct15m;
  final double minPct1h;
  final double minVolumeEur;
  final double trailingStopPct;
  final double hardStopLossPct;
  final int maxOpenTrades;
  final double stepUp1Profit;
  final double stepUp1Trailing;
  final double stepUp2Profit;
  final double stepUp2Trailing;

  const StrategyPreset({
    required this.name,
    required this.description,
    required this.icon,
    required this.color,
    required this.minPct24h,
    required this.minPct15m,
    required this.minPct1h,
    required this.minVolumeEur,
    required this.trailingStopPct,
    required this.hardStopLossPct,
    required this.maxOpenTrades,
    required this.stepUp1Profit,
    required this.stepUp1Trailing,
    required this.stepUp2Profit,
    required this.stepUp2Trailing,
  });

  static const conservative = StrategyPreset(
    name: 'Konservativ',
    description: 'Niedriges Risiko, stabile Gewinne. Ideal für Einsteiger.',
    icon: Icons.shield_outlined,
    color: Colors.blue,
    minPct24h: 8.0,
    minPct15m: 2.0,
    minPct1h: 3.0,
    minVolumeEur: 50000,
    trailingStopPct: 6.0,
    hardStopLossPct: 8.0,
    maxOpenTrades: 2,
    stepUp1Profit: 15.0,
    stepUp1Trailing: 8.0,
    stepUp2Profit: 30.0,
    stepUp2Trailing: 12.0,
  );

  static const moderate = StrategyPreset(
    name: 'Moderat',
    description: 'Ausgewogenes Risiko-Rendite-Verhältnis. Empfohlen.',
    icon: Icons.balance,
    color: Colors.teal,
    minPct24h: 5.0,
    minPct15m: 1.0,
    minPct1h: 2.0,
    minVolumeEur: 10000,
    trailingStopPct: 10.0,
    hardStopLossPct: 15.0,
    maxOpenTrades: 3,
    stepUp1Profit: 20.0,
    stepUp1Trailing: 15.0,
    stepUp2Profit: 50.0,
    stepUp2Trailing: 25.0,
  );

  static const aggressive = StrategyPreset(
    name: 'Aggressiv',
    description: 'Hohes Risiko, hohe Rendite. Nur für Erfahrene.',
    icon: Icons.rocket_launch,
    color: Colors.orange,
    minPct24h: 3.0,
    minPct15m: 0.5,
    minPct1h: 1.0,
    minVolumeEur: 5000,
    trailingStopPct: 15.0,
    hardStopLossPct: 25.0,
    maxOpenTrades: 5,
    stepUp1Profit: 25.0,
    stepUp1Trailing: 20.0,
    stepUp2Profit: 75.0,
    stepUp2Trailing: 35.0,
  );

  static StrategyPreset fromType(StrategyType type) {
    switch (type) {
      case StrategyType.conservative:
        return conservative;
      case StrategyType.moderate:
        return moderate;
      case StrategyType.aggressive:
        return aggressive;
      case StrategyType.custom:
        return moderate; // Default base for custom
    }
  }
}

// =============================================================================
// HELP TEXTS
// =============================================================================

class HelpTexts {
  static const minPct24h = '''
**24h Mindestanstieg**

Der minimale Preisanstieg in den letzten 24 Stunden, den ein Coin haben muss, um als Pump-Kandidat zu gelten.

• Niedrig (3-5%): Mehr Signale, höheres Risiko
• Mittel (5-8%): Ausgewogen
• Hoch (8%+): Weniger Signale, etablierte Trends
''';

  static const minVolume = '''
**Mindestvolumen**

Das minimale 24h-Handelsvolumen in EUR. Filtert illiquide Coins heraus.

• €5.000: Aggressive (Memecoins)
• €10.000: Standard
• €50.000+: Nur etablierte Coins
''';

  static const trailingStop = '''
**Trailing Stop-Loss**

Verkauft automatisch, wenn der Preis um X% vom Höchststand fällt.

Beispiel bei 10%:
• Kauf bei €1.00, Peak bei €1.50
• Verkauf wenn Preis auf €1.35 fällt (10% unter Peak)
• Gewinn: +35% statt potentiellem Verlust
''';

  static const hardStopLoss = '''
**Hard Stop-Loss**

Notfall-Verkauf bei festem Verlust. Schützt vor großen Verlusten.

• 8%: Konservativ, schneller Ausstieg
• 15%: Standard
• 25%: Aggressiv, mehr Spielraum
''';

  static const maxTrades = '''
**Max. offene Trades**

Maximale Anzahl gleichzeitiger Positionen.

• 1-2: Konzentriert, weniger Risiko
• 3-4: Diversifiziert
• 5+: Gestreut, erfordert mehr Kapital
''';

  static const stepUp = '''
**Step-Up Trailing Stop**

Dynamische Anpassung des Trailing Stops bei steigenden Gewinnen.

Beispiel:
• Basis: 10% Trailing
• Bei +20% Gewinn: Trailing auf 15%
• Bei +50% Gewinn: Trailing auf 25%

So können Gewinne weiterlaufen während Profits gesichert werden.
''';
}

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  await initializeService();
  runApp(const MatrixQuantApp());
}

// =============================================================================
// BACKGROUND SERVICE
// =============================================================================

Future<void> initializeService() async {
  final service = FlutterBackgroundService();

  await service.configure(
    androidConfiguration: AndroidConfiguration(
      onStart: onStart,
      autoStart: false,
      autoStartOnBoot: false,
      isForegroundMode: true,
      notificationChannelId: 'matrix_quant_channel',
      initialNotificationTitle: 'Matrix Quant',
      initialNotificationContent: 'Engine bereit',
      foregroundServiceNotificationId: 888,
      // Keep service alive when app is swiped away
      foregroundServiceTypes: [AndroidForegroundType.dataSync],
    ),
    iosConfiguration: IosConfiguration(
      autoStart: false,
      onForeground: onStart,
      onBackground: onIosBackground,
    ),
  );
}

@pragma('vm:entry-point')
Future<bool> onIosBackground(ServiceInstance service) async => true;

@pragma('vm:entry-point')
void onStart(ServiceInstance service) async {
  DartPluginRegistrant.ensureInitialized();
  await RustLib.init();

  if (service is AndroidServiceInstance) {
    service.on('setAsForeground').listen((_) => service.setAsForegroundService());
    service.on('setAsBackground').listen((_) => service.setAsBackgroundService());
  }
  service.on('stopService').listen((_) => service.stopSelf());

  // Read tick interval from storage (default 60s)
  const storage = FlutterSecureStorage();
  final intervalStr = await storage.read(key: 'tick_interval_sec') ?? '60';
  final intervalSec = int.tryParse(intervalStr) ?? 60;

  Timer.periodic(Duration(seconds: intervalSec), (timer) async {
    if (service is AndroidServiceInstance && await service.isForegroundService()) {
      try {
        final result = await runTick();
        service.setForegroundNotificationInfo(title: "Matrix Quant", content: result);

        // Persist state after each tick
        try {
          final stateJson = exportState();
          const storage = FlutterSecureStorage();
          await storage.write(key: 'persisted_trade_state', value: stateJson);
        } catch (e) {
          // Non-fatal: state persistence failure shouldn't stop trading
          log('State persistence error: $e');
        }
      } catch (e) {
        service.setForegroundNotificationInfo(title: "Matrix Quant", content: "Fehler: $e");
      }
    }
  });
}

// =============================================================================
// APP WIDGET
// =============================================================================

class MatrixQuantApp extends StatelessWidget {
  const MatrixQuantApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Matrix Quant',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        brightness: Brightness.dark,
        primarySwatch: Colors.teal,
        useMaterial3: true,
        scaffoldBackgroundColor: AppColors.background,
        cardColor: AppColors.surface,
        appBarTheme: const AppBarTheme(backgroundColor: Colors.transparent, elevation: 0),
        sliderTheme: SliderThemeData(
          activeTrackColor: AppColors.accent,
          thumbColor: AppColors.accent,
          overlayColor: AppColors.accent.withAlpha(50),
        ),
      ),
      home: const AuthScreen(),
    );
  }
}

// =============================================================================
// AUTH SCREEN
// =============================================================================

class AuthScreen extends StatefulWidget {
  const AuthScreen({super.key});
  @override
  State<AuthScreen> createState() => _AuthScreenState();
}

class _AuthScreenState extends State<AuthScreen> {
  final LocalAuthentication auth = LocalAuthentication();
  bool _isAuthenticating = false;
  String _error = '';

  @override
  void initState() {
    super.initState();
    _authenticate();
  }

  Future<void> _authenticate() async {
    try {
      setState(() { _isAuthenticating = true; _error = ''; });

      final canAuth = await auth.canCheckBiometrics || await auth.isDeviceSupported();
      if (!canAuth) { _navigate(); return; }

      final success = await auth.authenticate(
        localizedReason: 'Authentifizieren um Matrix Quant zu öffnen',
        biometricOnly: false,
        persistAcrossBackgrounding: true,
      );

      if (success) _navigate();
      else setState(() { _error = 'Authentifizierung fehlgeschlagen'; _isAuthenticating = false; });
    } on PlatformException catch (e) {
      setState(() { _error = e.message ?? 'Fehler'; _isAuthenticating = false; });
    }
  }

  void _navigate() {
    if (!mounted) return;
    Navigator.of(context).pushReplacement(MaterialPageRoute(builder: (_) => const MainScreen()));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
        decoration: BoxDecoration(
          gradient: RadialGradient(
            center: const Alignment(-0.5, -0.5),
            radius: 1.5,
            colors: [AppColors.surface, AppColors.background],
          ),
        ),
        child: Center(
          child: GlassCard(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _iconCircle(Icons.security, AppColors.accent),
                const SizedBox(height: 24),
                const Text('Matrix Quant', style: TextStyle(fontSize: 28, fontWeight: FontWeight.bold)),
                const SizedBox(height: 8),
                const Text('Momentum Trading Engine', style: TextStyle(color: Colors.grey)),
                const SizedBox(height: 24),
                if (_error.isNotEmpty) ...[
                  Text(_error, style: const TextStyle(color: AppColors.loss)),
                  const SizedBox(height: 16),
                ],
                if (_isAuthenticating)
                  const CircularProgressIndicator(color: AppColors.accent)
                else
                  ElevatedButton.icon(
                    onPressed: _authenticate,
                    icon: const Icon(Icons.fingerprint, size: 24),
                    label: const Text('Authentifizieren'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 32, vertical: 16),
                      backgroundColor: Colors.teal,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _iconCircle(IconData icon, Color color) {
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: color.withAlpha(30),
        boxShadow: [BoxShadow(color: color.withAlpha(50), blurRadius: 20)],
      ),
      child: Icon(icon, size: 48, color: color),
    );
  }
}

// =============================================================================
// MAIN SCREEN WITH BOTTOM NAVIGATION
// =============================================================================

class MainScreen extends StatefulWidget {
  const MainScreen({super.key});
  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  int _currentIndex = 0;

  final _screens = const [
    DashboardScreen(),
    TradesScreen(),
    SettingsScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _screens[_currentIndex],
      bottomNavigationBar: Container(
        decoration: BoxDecoration(
          color: AppColors.surface,
          border: Border(top: BorderSide(color: Colors.white.withAlpha(10))),
        ),
        child: BottomNavigationBar(
          currentIndex: _currentIndex,
          onTap: (i) => setState(() => _currentIndex = i),
          backgroundColor: Colors.transparent,
          selectedItemColor: AppColors.accent,
          unselectedItemColor: Colors.grey,
          elevation: 0,
          items: const [
            BottomNavigationBarItem(icon: Icon(Icons.dashboard), label: 'Dashboard'),
            BottomNavigationBarItem(icon: Icon(Icons.candlestick_chart), label: 'Trades'),
            BottomNavigationBarItem(icon: Icon(Icons.settings), label: 'Einstellungen'),
          ],
        ),
      ),
    );
  }
}

// =============================================================================
// DASHBOARD SCREEN
// =============================================================================

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});
  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  EngineStatusDto? _status;
  List<TradeDto> _trades = [];
  Timer? _timer;
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    _checkInit();
    _timer = Timer.periodic(const Duration(seconds: 3), (_) => _refresh());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _checkInit() {
    _initialized = isInitialized();
    if (_initialized) _refresh();
    setState(() {});
  }

  void _refresh() {
    if (!_initialized) return;
    try {
      _status = getStatus();
      _trades = getOpenTrades();
      setState(() {});
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: _gradientBg(),
      child: SafeArea(
        child: !_initialized ? _buildSetupPrompt() : _buildDashboard(),
      ),
    );
  }

  Widget _buildSetupPrompt() {
    return Center(
      child: GlassCard(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.rocket_launch, size: 64, color: AppColors.accent),
            const SizedBox(height: 20),
            const Text('Willkommen bei Matrix Quant', style: TextStyle(fontSize: 22, fontWeight: FontWeight.bold)),
            const SizedBox(height: 12),
            const Text(
              'Konfiguriere deine API-Zugangsdaten\nund Strategie in den Einstellungen.',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.grey),
            ),
            const SizedBox(height: 24),
            ElevatedButton.icon(
              onPressed: () {
                final mainState = context.findAncestorStateOfType<_MainScreenState>();
                mainState?.setState(() => mainState._currentIndex = 2);
              },
              icon: const Icon(Icons.settings),
              label: const Text('Einstellungen öffnen'),
              style: ElevatedButton.styleFrom(backgroundColor: Colors.teal),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDashboard() {
    final isRunning = _status?.isRunning ?? false;

    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Header
          Row(
            children: [
              const Text('Matrix Quant', style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold)),
              const Spacer(),
              _statusBadge(isRunning),
            ],
          ),
          const SizedBox(height: 20),

          // Stats Cards
          Row(
            children: [
              Expanded(child: _statCard('Balance', '€${_status?.eurBalance.toStringAsFixed(2) ?? "0.00"}', Icons.account_balance_wallet)),
              const SizedBox(width: 12),
              Expanded(child: _statCard('Paare', '${_status?.totalPairs ?? 0}', Icons.currency_exchange)),
              const SizedBox(width: 12),
              Expanded(child: _statCard('Trades', '${_status?.openTradesCount ?? 0}', Icons.trending_up)),
            ],
          ),
          const SizedBox(height: 20),

          // Control Buttons
          Row(
            children: [
              Expanded(
                child: ElevatedButton.icon(
                  onPressed: isRunning ? null : _start,
                  icon: const Icon(Icons.play_arrow),
                  label: const Text('Start'),
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 16),
                    backgroundColor: AppColors.profit,
                    disabledBackgroundColor: Colors.grey.withAlpha(50),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: ElevatedButton.icon(
                  onPressed: isRunning ? _stop : null,
                  icon: const Icon(Icons.stop),
                  label: const Text('Stop'),
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 16),
                    backgroundColor: AppColors.loss,
                    disabledBackgroundColor: Colors.grey.withAlpha(50),
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 24),

          // Open Trades
          if (_trades.isNotEmpty) ...[
            const Text('Offene Positionen', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
            const SizedBox(height: 12),
            ..._trades.map(_tradeCard),
          ] else ...[
            GlassCard(
              padding: const EdgeInsets.all(32),
              child: Column(
                children: [
                  Icon(Icons.hourglass_empty, size: 48, color: Colors.grey.withAlpha(100)),
                  const SizedBox(height: 12),
                  const Text('Keine offenen Trades', style: TextStyle(color: Colors.grey)),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _statusBadge(bool isRunning) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
      decoration: BoxDecoration(
        color: (isRunning ? AppColors.profit : AppColors.loss).withAlpha(30),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: (isRunning ? AppColors.profit : AppColors.loss).withAlpha(100)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 8, height: 8,
            decoration: BoxDecoration(
              shape: BoxShape.circle,
              color: isRunning ? AppColors.profit : AppColors.loss,
            ),
          ),
          const SizedBox(width: 8),
          Text(
            isRunning ? 'AKTIV' : 'GESTOPPT',
            style: TextStyle(
              color: isRunning ? AppColors.profit : AppColors.loss,
              fontWeight: FontWeight.bold,
              fontSize: 12,
            ),
          ),
        ],
      ),
    );
  }

  Widget _statCard(String label, String value, IconData icon) {
    return GlassCard(
      padding: const EdgeInsets.all(16),
      child: Column(
        children: [
          Icon(icon, color: AppColors.accent, size: 28),
          const SizedBox(height: 8),
          Text(value, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
          const SizedBox(height: 4),
          Text(label, style: const TextStyle(color: Colors.grey, fontSize: 12)),
        ],
      ),
    );
  }

  Widget _tradeCard(TradeDto trade) {
    final profit = trade.profitPct >= 0;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: GlassCard(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Container(
              width: 4, height: 50,
              decoration: BoxDecoration(
                color: profit ? AppColors.profit : AppColors.loss,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(trade.pair, style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
                  const SizedBox(height: 4),
                  Text(
                    'Einstieg: €${trade.entryPrice.toStringAsFixed(6)} • ${trade.timeInTradeMin} min',
                    style: const TextStyle(color: Colors.grey, fontSize: 12),
                  ),
                ],
              ),
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  '${profit ? "+" : ""}${trade.profitPct.toStringAsFixed(1)}%',
                  style: TextStyle(
                    color: profit ? AppColors.profit : AppColors.loss,
                    fontWeight: FontWeight.bold,
                    fontSize: 18,
                  ),
                ),
                Text(
                  '€${trade.profitEur.toStringAsFixed(2)}',
                  style: TextStyle(color: (profit ? AppColors.profit : AppColors.loss).withAlpha(180), fontSize: 12),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _start() async {
    try {
      await startEngine();
      FlutterBackgroundService().startService();
      _refresh();
      _showSnack('Engine gestartet', AppColors.profit);
    } catch (e) {
      _showSnack('Fehler: $e', AppColors.loss);
    }
  }

  void _stop() {
    try {
      stopEngine();
      FlutterBackgroundService().invoke('stopService');
      _refresh();
      _showSnack('Engine gestoppt', AppColors.warning);
    } catch (e) {
      _showSnack('Fehler: $e', AppColors.loss);
    }
  }

  void _showSnack(String msg, Color color) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(msg), backgroundColor: color),
    );
  }
}

// =============================================================================
// TRADES SCREEN
// =============================================================================

class TradesScreen extends StatefulWidget {
  const TradesScreen({super.key});
  @override
  State<TradesScreen> createState() => _TradesScreenState();
}

class _TradesScreenState extends State<TradesScreen> {
  List<TradeDto> _trades = [];

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  void _refresh() {
    try {
      _trades = getOpenTrades();
      setState(() {});
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: _gradientBg(),
      child: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  const Text('Offene Trades', style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold)),
                  const Spacer(),
                  IconButton(
                    onPressed: _refresh,
                    icon: const Icon(Icons.refresh, color: AppColors.accent),
                  ),
                ],
              ),
            ),
            Expanded(
              child: _trades.isEmpty
                  ? Center(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(Icons.inbox, size: 64, color: Colors.grey.withAlpha(100)),
                          const SizedBox(height: 16),
                          const Text('Keine offenen Trades', style: TextStyle(color: Colors.grey)),
                        ],
                      ),
                    )
                  : ListView.builder(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      itemCount: _trades.length,
                      itemBuilder: (_, i) => _detailedTradeCard(_trades[i]),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _detailedTradeCard(TradeDto trade) {
    final profit = trade.profitPct >= 0;
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: GlassCard(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Text(trade.pair, style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold)),
                const Spacer(),
                Text(
                  '${profit ? "+" : ""}${trade.profitPct.toStringAsFixed(2)}%',
                  style: TextStyle(
                    color: profit ? AppColors.profit : AppColors.loss,
                    fontWeight: FontWeight.bold,
                    fontSize: 24,
                  ),
                ),
              ],
            ),
            const Divider(height: 24),
            _infoRow('Einstiegspreis', '€${trade.entryPrice.toStringAsFixed(8)}'),
            _infoRow('Aktueller Preis', '€${trade.currentPrice.toStringAsFixed(8)}'),
            _infoRow('Menge', trade.amount.toStringAsFixed(6)),
            _infoRow('Investiert', '€${trade.stakeEur.toStringAsFixed(2)}'),
            _infoRow('Gewinn/Verlust', '€${trade.profitEur.toStringAsFixed(2)}'),
            _infoRow('Trailing Stop', '${trade.trailingStopPct.toStringAsFixed(1)}%'),
            _infoRow('Zeit im Trade', '${trade.timeInTradeMin} Minuten'),
          ],
        ),
      ),
    );
  }

  Widget _infoRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(label, style: const TextStyle(color: Colors.grey)),
          Text(value, style: const TextStyle(fontWeight: FontWeight.w500)),
        ],
      ),
    );
  }
}

// =============================================================================
// SETTINGS SCREEN (COMPREHENSIVE)
// =============================================================================

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});
  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> with SingleTickerProviderStateMixin {
  final _storage = const FlutterSecureStorage();
  late TabController _tabController;

  // API
  final _apiKeyCtrl = TextEditingController();
  final _apiSecretCtrl = TextEditingController();
  bool _obscureKey = true;
  bool _obscureSecret = true;

  // Strategy
  StrategyType _strategyType = StrategyType.moderate;
  bool _advancedMode = false;

  // Parameters
  double _minPct24h = 5.0;
  double _minPct15m = 1.0;
  double _minPct1h = 2.0;
  double _minVolume = 10000;
  double _trailingStop = 10.0;
  double _hardStopLoss = 15.0;
  int _maxTrades = 3;
  int _tickIntervalSec = 60;
  double _stepUp1Profit = 20.0;
  double _stepUp1Trailing = 15.0;
  double _stepUp2Profit = 50.0;
  double _stepUp2Trailing = 25.0;

  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _loadSettings();
  }

  @override
  void dispose() {
    _tabController.dispose();
    _apiKeyCtrl.dispose();
    _apiSecretCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadSettings() async {
    _apiKeyCtrl.text = await _storage.read(key: 'api_key') ?? '';
    _apiSecretCtrl.text = await _storage.read(key: 'api_secret') ?? '';

    final strategyStr = await _storage.read(key: 'strategy_type') ?? 'moderate';
    _strategyType = StrategyType.values.firstWhere((e) => e.name == strategyStr, orElse: () => StrategyType.moderate);
    _advancedMode = (await _storage.read(key: 'advanced_mode')) == 'true';

    _minPct24h = double.tryParse(await _storage.read(key: 'min_pct_24h') ?? '') ?? 5.0;
    _minPct15m = double.tryParse(await _storage.read(key: 'min_pct_15m') ?? '') ?? 1.0;
    _minPct1h = double.tryParse(await _storage.read(key: 'min_pct_1h') ?? '') ?? 2.0;
    _minVolume = double.tryParse(await _storage.read(key: 'min_volume') ?? '') ?? 10000;
    _trailingStop = double.tryParse(await _storage.read(key: 'trailing_stop') ?? '') ?? 10.0;
    _hardStopLoss = double.tryParse(await _storage.read(key: 'hard_stop_loss') ?? '') ?? 15.0;
    _maxTrades = int.tryParse(await _storage.read(key: 'max_trades') ?? '') ?? 3;
    _tickIntervalSec = int.tryParse(await _storage.read(key: 'tick_interval_sec') ?? '') ?? 60;
    _stepUp1Profit = double.tryParse(await _storage.read(key: 'step_up_1_profit') ?? '') ?? 20.0;
    _stepUp1Trailing = double.tryParse(await _storage.read(key: 'step_up_1_trailing') ?? '') ?? 15.0;
    _stepUp2Profit = double.tryParse(await _storage.read(key: 'step_up_2_profit') ?? '') ?? 50.0;
    _stepUp2Trailing = double.tryParse(await _storage.read(key: 'step_up_2_trailing') ?? '') ?? 25.0;

    setState(() => _loading = false);
  }

  void _applyPreset(StrategyPreset preset) {
    setState(() {
      _minPct24h = preset.minPct24h;
      _minPct15m = preset.minPct15m;
      _minPct1h = preset.minPct1h;
      _minVolume = preset.minVolumeEur;
      _trailingStop = preset.trailingStopPct;
      _hardStopLoss = preset.hardStopLossPct;
      _maxTrades = preset.maxOpenTrades;
      _stepUp1Profit = preset.stepUp1Profit;
      _stepUp1Trailing = preset.stepUp1Trailing;
      _stepUp2Profit = preset.stepUp2Profit;
      _stepUp2Trailing = preset.stepUp2Trailing;
    });
  }

  Future<void> _saveAndInit() async {
    if (_apiKeyCtrl.text.isEmpty || _apiSecretCtrl.text.isEmpty) {
      _showSnack('API Key und Secret erforderlich', AppColors.loss);
      return;
    }

    setState(() => _loading = true);

    try {
      // Save all settings
      await _storage.write(key: 'api_key', value: _apiKeyCtrl.text);
      await _storage.write(key: 'api_secret', value: _apiSecretCtrl.text);
      await _storage.write(key: 'strategy_type', value: _strategyType.name);
      await _storage.write(key: 'advanced_mode', value: _advancedMode.toString());
      await _storage.write(key: 'min_pct_24h', value: _minPct24h.toString());
      await _storage.write(key: 'min_pct_15m', value: _minPct15m.toString());
      await _storage.write(key: 'min_pct_1h', value: _minPct1h.toString());
      await _storage.write(key: 'min_volume', value: _minVolume.toString());
      await _storage.write(key: 'trailing_stop', value: _trailingStop.toString());
      await _storage.write(key: 'hard_stop_loss', value: _hardStopLoss.toString());
      await _storage.write(key: 'max_trades', value: _maxTrades.toString());
      await _storage.write(key: 'tick_interval_sec', value: _tickIntervalSec.toString());
      await _storage.write(key: 'step_up_1_profit', value: _stepUp1Profit.toString());
      await _storage.write(key: 'step_up_1_trailing', value: _stepUp1Trailing.toString());
      await _storage.write(key: 'step_up_2_profit', value: _stepUp2Profit.toString());
      await _storage.write(key: 'step_up_2_trailing', value: _stepUp2Trailing.toString());

      // Initialize engine
      final config = ConfigDto(
        apiKey: _apiKeyCtrl.text,
        apiSecret: _apiSecretCtrl.text,
        maxOpenTrades: _maxTrades,
        minPct24H: _minPct24h,
        minPct15M: _minPct15m,
        minPct1H: _minPct1h,
        minVolumeEur: _minVolume,
        trailingStopPct: _trailingStop,
        hardSlPct: -_hardStopLoss,
      );

      final result = await initializeEngine(config: config);

      // Restore persisted state if available
      final persistedState = await _storage.read(key: 'persisted_trade_state');
      if (persistedState != null && persistedState.isNotEmpty) {
        try {
          final importResult = await importState(stateJson: persistedState);
          log('State restored: $importResult');

          // Reconcile with Kraken to detect trades closed while offline
          final closedTrades = await reconcileState();
          if (closedTrades.isNotEmpty) {
            _showSnack('${closedTrades.length} Trade(s) wurden offline geschlossen', AppColors.warning);
          }
        } catch (e) {
          log('State recovery error: $e');
        }
      }

      // Sync existing Kraken positions (trades opened before or outside the bot)
      try {
        final synced = await syncExistingPositions();
        if (synced.isNotEmpty) {
          _showSnack('${synced.length} bestehende Position(en) erkannt', AppColors.accent);
        }
      } catch (e) {
        log('Position sync error: $e');
      }

      _showSnack(result, AppColors.profit);
    } catch (e) {
      _showSnack('Fehler: $e', AppColors.loss);
    } finally {
      setState(() => _loading = false);
    }
  }

  void _showSnack(String msg, Color color) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg), backgroundColor: color));
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return Container(
        decoration: _gradientBg(),
        child: const Center(child: CircularProgressIndicator(color: AppColors.accent)),
      );
    }

    return Container(
      decoration: _gradientBg(),
      child: SafeArea(
        child: Column(
          children: [
            // Header
            Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  const Text('Einstellungen', style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold)),
                  const Spacer(),
                  TextButton.icon(
                    onPressed: _saveAndInit,
                    icon: const Icon(Icons.save, color: AppColors.accent),
                    label: const Text('Speichern', style: TextStyle(color: AppColors.accent)),
                  ),
                ],
              ),
            ),

            // Tab Bar
            Container(
              margin: const EdgeInsets.symmetric(horizontal: 16),
              decoration: BoxDecoration(
                color: AppColors.surface,
                borderRadius: BorderRadius.circular(12),
              ),
              child: TabBar(
                controller: _tabController,
                indicator: BoxDecoration(
                  color: AppColors.accent.withAlpha(30),
                  borderRadius: BorderRadius.circular(12),
                ),
                labelColor: AppColors.accent,
                unselectedLabelColor: Colors.grey,
                tabs: const [
                  Tab(text: 'API'),
                  Tab(text: 'Strategie'),
                  Tab(text: 'Erweitert'),
                ],
              ),
            ),

            // Tab Views
            Expanded(
              child: TabBarView(
                controller: _tabController,
                children: [
                  _buildApiTab(),
                  _buildStrategyTab(),
                  _buildAdvancedTab(),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildApiTab() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _sectionTitle('Kraken API Zugangsdaten'),
          const SizedBox(height: 8),
          GlassCard(
            padding: const EdgeInsets.all(20),
            child: Column(
              children: [
                TextFormField(
                  controller: _apiKeyCtrl,
                  obscureText: _obscureKey,
                  decoration: InputDecoration(
                    labelText: 'API Key',
                    prefixIcon: const Icon(Icons.key),
                    suffixIcon: IconButton(
                      icon: Icon(_obscureKey ? Icons.visibility : Icons.visibility_off),
                      onPressed: () => setState(() => _obscureKey = !_obscureKey),
                    ),
                    border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
                  ),
                ),
                const SizedBox(height: 16),
                TextFormField(
                  controller: _apiSecretCtrl,
                  obscureText: _obscureSecret,
                  decoration: InputDecoration(
                    labelText: 'API Secret',
                    prefixIcon: const Icon(Icons.lock),
                    suffixIcon: IconButton(
                      icon: Icon(_obscureSecret ? Icons.visibility : Icons.visibility_off),
                      onPressed: () => setState(() => _obscureSecret = !_obscureSecret),
                    ),
                    border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 20),
          _infoBox(
            Icons.shield,
            'Sicherheit',
            'Deine API-Daten werden verschlüsselt auf dem Gerät gespeichert (AES-256).',
            Colors.blue,
          ),
          const SizedBox(height: 12),
          _infoBox(
            Icons.info_outline,
            'API Berechtigungen',
            'Aktiviere nur: Query Funds, Query Orders, Create Orders.\nNIEMALS: Withdraw Funds!',
            AppColors.warning,
          ),
        ],
      ),
    );
  }

  Widget _buildStrategyTab() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _sectionTitle('Strategie wählen'),
          const SizedBox(height: 12),

          // Strategy Cards
          _strategyCard(StrategyType.conservative, StrategyPreset.conservative),
          _strategyCard(StrategyType.moderate, StrategyPreset.moderate),
          _strategyCard(StrategyType.aggressive, StrategyPreset.aggressive),
          _strategyCard(StrategyType.custom, StrategyPreset(
            name: 'Benutzerdefiniert',
            description: 'Vollständige Kontrolle über alle Parameter.',
            icon: Icons.tune,
            color: Colors.purple,
            minPct24h: _minPct24h, minPct15m: _minPct15m, minPct1h: _minPct1h,
            minVolumeEur: _minVolume, trailingStopPct: _trailingStop,
            hardStopLossPct: _hardStopLoss, maxOpenTrades: _maxTrades,
            stepUp1Profit: _stepUp1Profit, stepUp1Trailing: _stepUp1Trailing,
            stepUp2Profit: _stepUp2Profit, stepUp2Trailing: _stepUp2Trailing,
          )),

          const SizedBox(height: 24),

          // Basic Parameters (always visible)
          _sectionTitle('Grundeinstellungen'),
          const SizedBox(height: 12),
          GlassCard(
            padding: const EdgeInsets.all(20),
            child: Column(
              children: [
                _paramSlider(
                  'Max. offene Trades',
                  _maxTrades.toDouble(), 1, 10,
                  (v) => setState(() => _maxTrades = v.round()),
                  helpText: HelpTexts.maxTrades,
                  valueFormat: (v) => '${v.round()} Trades',
                ),
                const Divider(height: 32),
                _paramSlider(
                  'Tick-Intervall',
                  _tickIntervalSec.toDouble(), 10, 300,
                  (v) => setState(() => _tickIntervalSec = v.round()),
                  helpText: '**Tick-Intervall**\n\nWie oft der Bot die Marktdaten abfragt.\n\n• 10-30s: Schnell, mehr API-Calls\n• 60s: Standard\n• 120-300s: Stromsparend',
                  valueFormat: (v) => '${v.round()}s',
                ),
                const Divider(height: 32),
                _paramSlider(
                  'Trailing Stop-Loss',
                  _trailingStop, 3, 30,
                  (v) => setState(() => _trailingStop = v),
                  helpText: HelpTexts.trailingStop,
                  valueFormat: (v) => '${v.round()}%',
                ),
                const Divider(height: 32),
                _paramSlider(
                  'Hard Stop-Loss',
                  _hardStopLoss, 5, 35,
                  (v) => setState(() => _hardStopLoss = v),
                  helpText: HelpTexts.hardStopLoss,
                  valueFormat: (v) => '${v.round()}%',
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildAdvancedTab() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Advanced Mode Toggle
          GlassCard(
            padding: const EdgeInsets.all(16),
            child: SwitchListTile(
              title: const Text('Erweiterte Einstellungen'),
              subtitle: const Text('Zeigt alle Parameter für Experten'),
              value: _advancedMode,
              onChanged: (v) => setState(() => _advancedMode = v),
              activeColor: AppColors.accent,
            ),
          ),

          if (_advancedMode) ...[
            const SizedBox(height: 20),

            // Entry Settings
            _sectionTitle('Einstiegs-Kriterien'),
            const SizedBox(height: 12),
            GlassCard(
              padding: const EdgeInsets.all(20),
              child: Column(
                children: [
                  _paramSlider(
                    '24h Mindestanstieg',
                    _minPct24h, 1, 20,
                    (v) => setState(() => _minPct24h = v),
                    helpText: HelpTexts.minPct24h,
                    valueFormat: (v) => '${v.round()}%',
                  ),
                  const Divider(height: 32),
                  _paramSlider(
                    '15min Mindestanstieg',
                    _minPct15m, 0, 5,
                    (v) => setState(() => _minPct15m = v),
                    valueFormat: (v) => '${v.toStringAsFixed(1)}%',
                  ),
                  const Divider(height: 32),
                  _paramSlider(
                    '1h Mindestanstieg',
                    _minPct1h, 0, 10,
                    (v) => setState(() => _minPct1h = v),
                    valueFormat: (v) => '${v.toStringAsFixed(1)}%',
                  ),
                  const Divider(height: 32),
                  _paramSlider(
                    'Mindestvolumen',
                    _minVolume, 1000, 100000,
                    (v) => setState(() => _minVolume = v),
                    helpText: HelpTexts.minVolume,
                    valueFormat: (v) => '€${(v/1000).round()}k',
                    divisions: 99,
                  ),
                ],
              ),
            ),

            const SizedBox(height: 20),

            // Step-Up Settings
            _sectionTitle('Step-Up Trailing Stop'),
            _helpButton(HelpTexts.stepUp),
            const SizedBox(height: 12),
            GlassCard(
              padding: const EdgeInsets.all(20),
              child: Column(
                children: [
                  const Text('Stufe 1', style: TextStyle(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 12),
                  _paramSlider(
                    'Bei Gewinn von',
                    _stepUp1Profit, 10, 50,
                    (v) => setState(() => _stepUp1Profit = v),
                    valueFormat: (v) => '+${v.round()}%',
                  ),
                  _paramSlider(
                    'Trailing wird',
                    _stepUp1Trailing, 5, 25,
                    (v) => setState(() => _stepUp1Trailing = v),
                    valueFormat: (v) => '${v.round()}%',
                  ),
                  const Divider(height: 24),
                  const Text('Stufe 2 (Moon Phase)', style: TextStyle(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 12),
                  _paramSlider(
                    'Bei Gewinn von',
                    _stepUp2Profit, 30, 100,
                    (v) => setState(() => _stepUp2Profit = v),
                    valueFormat: (v) => '+${v.round()}%',
                  ),
                  _paramSlider(
                    'Trailing wird',
                    _stepUp2Trailing, 15, 50,
                    (v) => setState(() => _stepUp2Trailing = v),
                    valueFormat: (v) => '${v.round()}%',
                  ),
                ],
              ),
            ),
          ] else ...[
            const SizedBox(height: 40),
            Center(
              child: Column(
                children: [
                  Icon(Icons.tune, size: 64, color: Colors.grey.withAlpha(100)),
                  const SizedBox(height: 16),
                  const Text(
                    'Aktiviere den Schalter oben,\num erweiterte Einstellungen zu sehen.',
                    textAlign: TextAlign.center,
                    style: TextStyle(color: Colors.grey),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _strategyCard(StrategyType type, StrategyPreset preset) {
    final selected = _strategyType == type;
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: GestureDetector(
        onTap: () {
          setState(() => _strategyType = type);
          if (type != StrategyType.custom) {
            _applyPreset(preset);
          }
        },
        child: Container(
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: selected ? preset.color.withAlpha(30) : AppColors.surface.withAlpha(150),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(
              color: selected ? preset.color : Colors.white.withAlpha(10),
              width: selected ? 2 : 1,
            ),
          ),
          child: Row(
            children: [
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: preset.color.withAlpha(30),
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(preset.icon, color: preset.color),
              ),
              const SizedBox(width: 16),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(preset.name, style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
                    const SizedBox(height: 4),
                    Text(preset.description, style: const TextStyle(color: Colors.grey, fontSize: 12)),
                  ],
                ),
              ),
              if (selected)
                Icon(Icons.check_circle, color: preset.color),
            ],
          ),
        ),
      ),
    );
  }

  Widget _paramSlider(
    String label,
    double value,
    double min,
    double max,
    ValueChanged<double> onChanged, {
    String? helpText,
    String Function(double)? valueFormat,
    int? divisions,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(child: Text(label, style: const TextStyle(color: Colors.grey))),
            if (helpText != null) _helpButton(helpText),
            const SizedBox(width: 8),
            Text(
              valueFormat?.call(value) ?? value.toStringAsFixed(1),
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
          ],
        ),
        Slider(
          value: value.clamp(min, max),
          min: min,
          max: max,
          divisions: divisions ?? (max - min).round(),
          onChanged: onChanged,
        ),
      ],
    );
  }

  Widget _helpButton(String helpText) {
    return IconButton(
      icon: const Icon(Icons.help_outline, size: 20, color: Colors.grey),
      onPressed: () => _showHelpDialog(helpText),
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints(),
    );
  }

  void _showHelpDialog(String text) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppColors.surface,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
        title: const Row(
          children: [
            Icon(Icons.info, color: AppColors.accent),
            SizedBox(width: 12),
            Text('Info'),
          ],
        ),
        content: SingleChildScrollView(
          child: Text(text, style: const TextStyle(height: 1.5)),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Verstanden', style: TextStyle(color: AppColors.accent)),
          ),
        ],
      ),
    );
  }

  Widget _sectionTitle(String title) {
    return Text(title, style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: AppColors.accent));
  }

  Widget _infoBox(IconData icon, String title, String text, Color color) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: color.withAlpha(20),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color.withAlpha(50)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, color: color),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: TextStyle(fontWeight: FontWeight.bold, color: color)),
                const SizedBox(height: 4),
                Text(text, style: TextStyle(color: color.withAlpha(200), fontSize: 12)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

// =============================================================================
// REUSABLE WIDGETS
// =============================================================================

class GlassCard extends StatelessWidget {
  final Widget child;
  final EdgeInsets padding;

  const GlassCard({super.key, required this.child, this.padding = const EdgeInsets.all(20)});

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(16),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
        child: Container(
          padding: padding,
          decoration: BoxDecoration(
            color: Colors.white.withAlpha(12),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: Colors.white.withAlpha(15)),
          ),
          child: child,
        ),
      ),
    );
  }
}

BoxDecoration _gradientBg() {
  return const BoxDecoration(
    gradient: RadialGradient(
      center: Alignment(0.3, -0.5),
      radius: 1.5,
      colors: [AppColors.surface, AppColors.background],
    ),
  );
}
