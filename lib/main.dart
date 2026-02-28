import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:matrix_quant_app/src/rust/frb_generated.dart';
import 'package:matrix_quant_app/src/rust/api/simple.dart';
import 'package:local_auth/local_auth.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_background_service/flutter_background_service.dart';
import 'dart:async';
import 'dart:ui';

// =============================================================================
// Main Entry Point
// =============================================================================

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  await initializeService();
  runApp(const MatrixQuantApp());
}

// =============================================================================
// Background Service Configuration
// =============================================================================

Future<void> initializeService() async {
  final service = FlutterBackgroundService();

  await service.configure(
    androidConfiguration: AndroidConfiguration(
      onStart: onStart,
      autoStart: false, // Manual start after config
      isForegroundMode: true,
      notificationChannelId: 'matrix_quant_channel',
      initialNotificationTitle: 'Matrix Quant Engine',
      initialNotificationContent: 'Initializing...',
      foregroundServiceNotificationId: 888,
    ),
    iosConfiguration: IosConfiguration(
      autoStart: false,
      onForeground: onStart,
      onBackground: onIosBackground,
    ),
  );
}

@pragma('vm:entry-point')
Future<bool> onIosBackground(ServiceInstance service) async {
  return true;
}

@pragma('vm:entry-point')
void onStart(ServiceInstance service) async {
  DartPluginRegistrant.ensureInitialized();
  await RustLib.init();

  if (service is AndroidServiceInstance) {
    service.on('setAsForeground').listen((event) {
      service.setAsForegroundService();
    });
    service.on('setAsBackground').listen((event) {
      service.setAsBackgroundService();
    });
  }

  service.on('stopService').listen((event) {
    service.stopSelf();
  });

  // Trading loop - 60 second intervals
  Timer.periodic(const Duration(seconds: 60), (timer) async {
    if (service is AndroidServiceInstance) {
      if (await service.isForegroundService()) {
        try {
          final result = await runTick();
          service.setForegroundNotificationInfo(
            title: "Matrix Quant Active",
            content: result,
          );
        } catch (e) {
          service.setForegroundNotificationInfo(
            title: "Matrix Quant Error",
            content: "Error: $e",
          );
        }
      }
    }
  });
}

// =============================================================================
// App Theme & Main Widget
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
        scaffoldBackgroundColor: const Color(0xFF0F172A),
        cardColor: const Color(0xFF1E293B),
        appBarTheme: const AppBarTheme(
          backgroundColor: Colors.transparent,
          elevation: 0,
        ),
      ),
      home: const AuthScreen(),
    );
  }
}

// =============================================================================
// Authentication Screen
// =============================================================================

class AuthScreen extends StatefulWidget {
  const AuthScreen({super.key});

  @override
  State<AuthScreen> createState() => _AuthScreenState();
}

class _AuthScreenState extends State<AuthScreen> {
  final LocalAuthentication auth = LocalAuthentication();
  bool _isAuthenticating = false;
  String _errorMessage = '';

  @override
  void initState() {
    super.initState();
    _authenticate();
  }

  Future<void> _authenticate() async {
    try {
      setState(() {
        _isAuthenticating = true;
        _errorMessage = '';
      });

      final canAuth = await auth.canCheckBiometrics || await auth.isDeviceSupported();

      if (!canAuth) {
        // No biometrics, go directly to dashboard
        _navigateToDashboard();
        return;
      }

      final authenticated = await auth.authenticate(
        localizedReason: 'Authenticate to access Matrix Quant',
        options: const AuthenticationOptions(
          stickyAuth: true,
          biometricOnly: false,
        ),
      );

      if (authenticated) {
        _navigateToDashboard();
      } else {
        setState(() {
          _errorMessage = 'Authentication failed';
          _isAuthenticating = false;
        });
      }
    } on PlatformException catch (e) {
      setState(() {
        _errorMessage = e.message ?? 'Authentication error';
        _isAuthenticating = false;
      });
    }
  }

  void _navigateToDashboard() {
    if (!mounted) return;
    Navigator.of(context).pushReplacement(
      MaterialPageRoute(builder: (context) => const DashboardScreen()),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: RadialGradient(
            center: Alignment(-0.8, -0.6),
            radius: 1.5,
            colors: [Color(0xFF1E293B), Color(0xFF0F172A)],
          ),
        ),
        child: Center(
          child: GlassCard(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                _buildIcon(Icons.security, Colors.tealAccent),
                const SizedBox(height: 30),
                const Text(
                  'Matrix Quant',
                  style: TextStyle(
                    fontSize: 28,
                    fontWeight: FontWeight.bold,
                    letterSpacing: 1.2,
                  ),
                ),
                const SizedBox(height: 8),
                const Text(
                  'Momentum Trading Engine',
                  style: TextStyle(fontSize: 14, color: Colors.grey),
                ),
                const SizedBox(height: 30),
                if (_errorMessage.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 20),
                    child: Text(
                      _errorMessage,
                      style: const TextStyle(color: Colors.redAccent),
                    ),
                  ),
                if (_isAuthenticating)
                  const CircularProgressIndicator(color: Colors.tealAccent)
                else
                  ElevatedButton.icon(
                    onPressed: _authenticate,
                    icon: const Icon(Icons.fingerprint, size: 28),
                    label: const Text('Authenticate'),
                    style: ElevatedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 40, vertical: 18),
                      backgroundColor: Colors.teal,
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(15),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildIcon(IconData icon, Color color) {
    return Container(
      padding: const EdgeInsets.all(24),
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: color.withAlpha(25),
        boxShadow: [
          BoxShadow(color: color.withAlpha(50), blurRadius: 20, spreadRadius: 2),
        ],
      ),
      child: Icon(icon, size: 60, color: color),
    );
  }
}

// =============================================================================
// Dashboard Screen
// =============================================================================

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({super.key});

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  EngineStatusDto? _status;
  List<TradeDto> _trades = [];
  Timer? _refreshTimer;
  bool _isInitialized = false;

  @override
  void initState() {
    super.initState();
    _checkInitialization();
    _startRefreshTimer();
  }

  @override
  void dispose() {
    _refreshTimer?.cancel();
    super.dispose();
  }

  void _startRefreshTimer() {
    _refreshTimer = Timer.periodic(const Duration(seconds: 5), (_) {
      _refreshStatus();
    });
  }

  Future<void> _checkInitialization() async {
    final initialized = isInitialized();
    setState(() {
      _isInitialized = initialized;
    });
    if (initialized) {
      _refreshStatus();
    }
  }

  Future<void> _refreshStatus() async {
    if (!_isInitialized) return;

    try {
      final status = getStatus();
      final trades = getOpenTrades();
      setState(() {
        _status = status;
        _trades = trades;
      });
    } catch (e) {
      debugPrint('Refresh error: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      extendBodyBehindAppBar: true,
      appBar: AppBar(
        title: const Text(
          'Matrix Quant',
          style: TextStyle(fontWeight: FontWeight.bold, letterSpacing: 1.2),
        ),
        actions: [
          IconButton(
            icon: const Icon(Icons.settings, color: Colors.tealAccent),
            onPressed: () => _openSettings(context),
          ),
        ],
      ),
      body: Container(
        decoration: const BoxDecoration(
          gradient: RadialGradient(
            center: Alignment(0.5, -0.8),
            radius: 1.8,
            colors: [Color(0xFF1E293B), Color(0xFF0F172A)],
          ),
        ),
        child: SafeArea(
          child: _isInitialized ? _buildDashboard() : _buildSetupPrompt(),
        ),
      ),
    );
  }

  Widget _buildSetupPrompt() {
    return Center(
      child: GlassCard(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.settings_outlined, size: 60, color: Colors.tealAccent),
            const SizedBox(height: 20),
            const Text(
              'Setup Required',
              style: TextStyle(fontSize: 22, fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 10),
            const Text(
              'Configure your Kraken API credentials\nto start trading.',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.grey),
            ),
            const SizedBox(height: 30),
            ElevatedButton.icon(
              onPressed: () => _openSettings(context),
              icon: const Icon(Icons.settings),
              label: const Text('Open Settings'),
              style: ElevatedButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 30, vertical: 15),
                backgroundColor: Colors.teal,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDashboard() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // Status Card
          GlassCard(
            child: Column(
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    const Text('Engine Status', style: TextStyle(color: Colors.grey)),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                      decoration: BoxDecoration(
                        color: (_status?.isRunning ?? false)
                            ? Colors.green.withAlpha(50)
                            : Colors.red.withAlpha(50),
                        borderRadius: BorderRadius.circular(20),
                      ),
                      child: Text(
                        (_status?.isRunning ?? false) ? 'RUNNING' : 'STOPPED',
                        style: TextStyle(
                          color: (_status?.isRunning ?? false) ? Colors.green : Colors.red,
                          fontWeight: FontWeight.bold,
                          fontSize: 12,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 20),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceAround,
                  children: [
                    _buildStatItem('Balance', '€${_status?.eurBalance.toStringAsFixed(2) ?? "0.00"}'),
                    _buildStatItem('Pairs', '${_status?.totalPairs ?? 0}'),
                    _buildStatItem('Trades', '${_status?.openTradesCount ?? 0}'),
                  ],
                ),
              ],
            ),
          ),

          const SizedBox(height: 20),

          // Open Trades
          if (_trades.isNotEmpty) ...[
            const Padding(
              padding: EdgeInsets.only(left: 8, bottom: 10),
              child: Text('Open Trades', style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
            ),
            ..._trades.map((trade) => _buildTradeCard(trade)),
          ],

          const SizedBox(height: 20),

          // Control Buttons
          Row(
            children: [
              Expanded(
                child: ElevatedButton.icon(
                  onPressed: _startEngine,
                  icon: const Icon(Icons.play_arrow),
                  label: const Text('Start'),
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 15),
                    backgroundColor: Colors.green.withAlpha(200),
                  ),
                ),
              ),
              const SizedBox(width: 15),
              Expanded(
                child: ElevatedButton.icon(
                  onPressed: _stopEngine,
                  icon: const Icon(Icons.stop),
                  label: const Text('Stop'),
                  style: ElevatedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 15),
                    backgroundColor: Colors.red.withAlpha(200),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildStatItem(String label, String value) {
    return Column(
      children: [
        Text(value, style: const TextStyle(fontSize: 24, fontWeight: FontWeight.bold)),
        const SizedBox(height: 4),
        Text(label, style: const TextStyle(color: Colors.grey, fontSize: 12)),
      ],
    );
  }

  Widget _buildTradeCard(TradeDto trade) {
    final isProfit = trade.profitPct >= 0;
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: GlassCard(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(trade.pair, style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
                  const SizedBox(height: 4),
                  Text(
                    'Entry: €${trade.entryPrice.toStringAsFixed(6)} | ${trade.timeInTradeMin}min',
                    style: const TextStyle(color: Colors.grey, fontSize: 12),
                  ),
                ],
              ),
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  '${isProfit ? "+" : ""}${trade.profitPct.toStringAsFixed(1)}%',
                  style: TextStyle(
                    color: isProfit ? Colors.green : Colors.red,
                    fontWeight: FontWeight.bold,
                    fontSize: 18,
                  ),
                ),
                Text(
                  '€${trade.profitEur.toStringAsFixed(2)}',
                  style: TextStyle(
                    color: isProfit ? Colors.green.withAlpha(180) : Colors.red.withAlpha(180),
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _startEngine() async {
    try {
      await startEngine();
      FlutterBackgroundService().startService();
      _refreshStatus();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Engine started'), backgroundColor: Colors.green),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  Future<void> _stopEngine() async {
    try {
      stopEngine();
      FlutterBackgroundService().invoke('stopService');
      _refreshStatus();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Engine stopped'), backgroundColor: Colors.orange),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    }
  }

  void _openSettings(BuildContext context) async {
    await Navigator.of(context).push(
      MaterialPageRoute(builder: (context) => const SettingsScreen()),
    );
    _checkInitialization();
  }
}

// =============================================================================
// Settings Screen
// =============================================================================

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _storage = const FlutterSecureStorage();
  final _formKey = GlobalKey<FormState>();

  final _apiKeyController = TextEditingController();
  final _apiSecretController = TextEditingController();

  double _maxTrades = 3;
  double _minPct24h = 5.0;
  double _trailingStop = 10.0;
  double _hardStopLoss = 15.0;
  double _minVolume = 10000;

  bool _isLoading = true;
  bool _obscureKey = true;
  bool _obscureSecret = true;

  @override
  void initState() {
    super.initState();
    _loadSettings();
  }

  @override
  void dispose() {
    _apiKeyController.dispose();
    _apiSecretController.dispose();
    super.dispose();
  }

  Future<void> _loadSettings() async {
    final apiKey = await _storage.read(key: 'kraken_api_key') ?? '';
    final apiSecret = await _storage.read(key: 'kraken_api_secret') ?? '';
    final maxTrades = await _storage.read(key: 'max_trades') ?? '3';
    final minPct24h = await _storage.read(key: 'min_pct_24h') ?? '5.0';
    final trailingStop = await _storage.read(key: 'trailing_stop') ?? '10.0';
    final hardStopLoss = await _storage.read(key: 'hard_stop_loss') ?? '15.0';
    final minVolume = await _storage.read(key: 'min_volume') ?? '10000';

    setState(() {
      _apiKeyController.text = apiKey;
      _apiSecretController.text = apiSecret;
      _maxTrades = double.tryParse(maxTrades) ?? 3;
      _minPct24h = double.tryParse(minPct24h) ?? 5.0;
      _trailingStop = double.tryParse(trailingStop) ?? 10.0;
      _hardStopLoss = double.tryParse(hardStopLoss) ?? 15.0;
      _minVolume = double.tryParse(minVolume) ?? 10000;
      _isLoading = false;
    });
  }

  Future<void> _saveAndInitialize() async {
    if (!_formKey.currentState!.validate()) return;

    setState(() => _isLoading = true);

    try {
      // Save to secure storage
      await _storage.write(key: 'kraken_api_key', value: _apiKeyController.text);
      await _storage.write(key: 'kraken_api_secret', value: _apiSecretController.text);
      await _storage.write(key: 'max_trades', value: _maxTrades.round().toString());
      await _storage.write(key: 'min_pct_24h', value: _minPct24h.toString());
      await _storage.write(key: 'trailing_stop', value: _trailingStop.toString());
      await _storage.write(key: 'hard_stop_loss', value: _hardStopLoss.toString());
      await _storage.write(key: 'min_volume', value: _minVolume.toString());

      // Initialize Rust engine
      final config = ConfigDto(
        apiKey: _apiKeyController.text,
        apiSecret: _apiSecretController.text,
        maxOpenTrades: _maxTrades.round(),
        minPct24h: _minPct24h,
        minPct15m: 1.0,
        minPct1h: 2.0,
        minVolumeEur: _minVolume,
        trailingStopPct: _trailingStop,
        hardSlPct: -_hardStopLoss,
      );

      final result = await initializeEngine(config: config);

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(result), backgroundColor: Colors.green),
        );
        Navigator.of(context).pop();
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error: $e'), backgroundColor: Colors.red),
        );
      }
    } finally {
      setState(() => _isLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => Navigator.of(context).pop(),
        ),
      ),
      body: _isLoading
          ? const Center(child: CircularProgressIndicator())
          : Container(
              decoration: const BoxDecoration(
                gradient: RadialGradient(
                  center: Alignment(0.5, -0.8),
                  radius: 1.8,
                  colors: [Color(0xFF1E293B), Color(0xFF0F172A)],
                ),
              ),
              child: Form(
                key: _formKey,
                child: ListView(
                  padding: const EdgeInsets.all(20),
                  children: [
                    // API Credentials Section
                    _buildSectionHeader('Kraken API Credentials'),
                    const SizedBox(height: 10),
                    GlassCard(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        children: [
                          TextFormField(
                            controller: _apiKeyController,
                            obscureText: _obscureKey,
                            decoration: InputDecoration(
                              labelText: 'API Key',
                              prefixIcon: const Icon(Icons.key),
                              suffixIcon: IconButton(
                                icon: Icon(_obscureKey ? Icons.visibility : Icons.visibility_off),
                                onPressed: () => setState(() => _obscureKey = !_obscureKey),
                              ),
                            ),
                            validator: (v) => v!.isEmpty ? 'Required' : null,
                          ),
                          const SizedBox(height: 15),
                          TextFormField(
                            controller: _apiSecretController,
                            obscureText: _obscureSecret,
                            decoration: InputDecoration(
                              labelText: 'API Secret',
                              prefixIcon: const Icon(Icons.lock),
                              suffixIcon: IconButton(
                                icon: Icon(_obscureSecret ? Icons.visibility : Icons.visibility_off),
                                onPressed: () => setState(() => _obscureSecret = !_obscureSecret),
                              ),
                            ),
                            validator: (v) => v!.isEmpty ? 'Required' : null,
                          ),
                        ],
                      ),
                    ),

                    const SizedBox(height: 25),

                    // Strategy Settings
                    _buildSectionHeader('Strategy Settings'),
                    const SizedBox(height: 10),
                    GlassCard(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        children: [
                          _buildSlider('Max Open Trades', _maxTrades, 1, 10, (v) {
                            setState(() => _maxTrades = v);
                          }, suffix: ' trades'),
                          const SizedBox(height: 20),
                          _buildSlider('Min 24h Pump', _minPct24h, 1, 20, (v) {
                            setState(() => _minPct24h = v);
                          }, suffix: '%'),
                          const SizedBox(height: 20),
                          _buildSlider('Trailing Stop', _trailingStop, 5, 30, (v) {
                            setState(() => _trailingStop = v);
                          }, suffix: '%'),
                          const SizedBox(height: 20),
                          _buildSlider('Hard Stop-Loss', _hardStopLoss, 5, 30, (v) {
                            setState(() => _hardStopLoss = v);
                          }, suffix: '%'),
                          const SizedBox(height: 20),
                          _buildSlider('Min Volume', _minVolume, 1000, 100000, (v) {
                            setState(() => _minVolume = v);
                          }, suffix: ' EUR', divisions: 99),
                        ],
                      ),
                    ),

                    const SizedBox(height: 30),

                    // Save Button
                    ElevatedButton.icon(
                      onPressed: _saveAndInitialize,
                      icon: const Icon(Icons.save),
                      label: const Text('Save & Initialize Engine'),
                      style: ElevatedButton.styleFrom(
                        padding: const EdgeInsets.symmetric(vertical: 18),
                        backgroundColor: Colors.teal,
                        foregroundColor: Colors.white,
                        shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(12),
                        ),
                      ),
                    ),

                    const SizedBox(height: 20),

                    // Security Notice
                    Container(
                      padding: const EdgeInsets.all(16),
                      decoration: BoxDecoration(
                        color: Colors.blue.withAlpha(20),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: Colors.blue.withAlpha(50)),
                      ),
                      child: const Row(
                        children: [
                          Icon(Icons.shield, color: Colors.blue),
                          SizedBox(width: 12),
                          Expanded(
                            child: Text(
                              'API credentials are stored encrypted on your device using Flutter Secure Storage.',
                              style: TextStyle(color: Colors.blue, fontSize: 12),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
    );
  }

  Widget _buildSectionHeader(String title) {
    return Text(
      title,
      style: const TextStyle(
        fontSize: 18,
        fontWeight: FontWeight.bold,
        color: Colors.tealAccent,
      ),
    );
  }

  Widget _buildSlider(
    String label,
    double value,
    double min,
    double max,
    ValueChanged<double> onChanged, {
    String suffix = '',
    int? divisions,
  }) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(label, style: const TextStyle(color: Colors.grey)),
            Text(
              '${value.round()}$suffix',
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
          ],
        ),
        Slider(
          value: value,
          min: min,
          max: max,
          divisions: divisions ?? (max - min).round(),
          activeColor: Colors.tealAccent,
          onChanged: onChanged,
        ),
      ],
    );
  }
}

// =============================================================================
// Reusable Glass Card Widget
// =============================================================================

class GlassCard extends StatelessWidget {
  final Widget child;
  final EdgeInsets padding;

  const GlassCard({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(24),
  });

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(20),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 10, sigmaY: 10),
        child: Container(
          padding: padding,
          decoration: BoxDecoration(
            color: Colors.white.withAlpha(15),
            borderRadius: BorderRadius.circular(20),
            border: Border.all(color: Colors.white.withAlpha(20)),
          ),
          child: child,
        ),
      ),
    );
  }
}
