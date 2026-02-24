import 'package:flutter/material.dart';
import 'package:matrix_quant_app/src/rust/frb_generated.dart';
import 'package:matrix_quant_app/src/rust/api/simple.dart';
import 'package:local_auth/local_auth.dart';
import 'package:flutter/services.dart';
import 'package:flutter_background_service/flutter_background_service.dart';
import 'dart:async';
import 'dart:ui';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  await initializeService();
  runApp(const MatrixQuantApp());
}

Future<void> initializeService() async {
  final service = FlutterBackgroundService();

  await service.configure(
    androidConfiguration: AndroidConfiguration(
      onStart: onStart,
      autoStart: true,
      isForegroundMode: true,
      notificationChannelId: 'matrix_quant_channel',
      initialNotificationTitle: 'Matrix Quant Engine',
      initialNotificationContent: 'Initializing Engine...',
      foregroundServiceNotificationId: 888,
    ),
    iosConfiguration: IosConfiguration(
      autoStart: true,
      onForeground: onStart,
      onBackground: onIosBackground,
    ),
  );
  
  service.startService();
}

@pragma('vm:entry-point')
Future<bool> onIosBackground(ServiceInstance service) async {
  return true;
}

@pragma('vm:entry-point')
void onStart(ServiceInstance service) async {
  DartPluginRegistrant.ensureInitialized();
  await RustLib.init(); // Re-init bridge for the background isolate

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

  // Main Background Loop
  Timer.periodic(const Duration(seconds: 60), (timer) async {
    if (service is AndroidServiceInstance) {
      if (await service.isForegroundService()) {
        try {
          // Call to Rust Engine using flutter_rust_bridge
          final status = greet(name: "Background Worker");
          service.setForegroundNotificationInfo(
            title: "Matrix Quant Engine Active",
            content: "Status: \$status",
          );
        } catch (e) {
             service.setForegroundNotificationInfo(
              title: "Matrix Quant Engine Error",
              content: "Rust panic or network error: \$e",
            );
        }
      }
    }
  });
}

class MatrixQuantApp extends StatelessWidget {
  const MatrixQuantApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Matrix Quant',
      theme: ThemeData(
        brightness: Brightness.dark,
        primarySwatch: Colors.teal,
        useMaterial3: true,
        scaffoldBackgroundColor: const Color(0xFF0F172A),
      ),
      home: const AuthScreen(),
    );
  }
}

class AuthScreen extends StatefulWidget {
  const AuthScreen({Key? key}) : super(key: key);

  @override
  State<AuthScreen> createState() => _AuthScreenState();
}

class _AuthScreenState extends State<AuthScreen> {
  final LocalAuthentication auth = LocalAuthentication();
  bool _isAuthenticating = false;

  @override
  void initState() {
    super.initState();
    _authenticate();
  }

  Future<void> _authenticate() async {
    bool authenticated = false;
    try {
      setState(() {
        _isAuthenticating = true;
      });
      authenticated = await auth.authenticate(
        localizedReason: 'Scan Fingerprint or Face to unlock',
      );
      setState(() {
        _isAuthenticating = false;
      });
      
      if (authenticated) {
        if (!mounted) return;
        Navigator.of(context).pushReplacement(
          MaterialPageRoute(builder: (context) => const DashboardScreen()),
        );
      }
    } on PlatformException catch (_) {
      setState(() {
        _isAuthenticating = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Stack(
        children: [
          // Background Gradient
          Container(
            decoration: const BoxDecoration(
              gradient: RadialGradient(
                center: Alignment(-0.8, -0.6),
                radius: 1.5,
                colors: [Color(0xFF1E293B), Color(0xFF0F172A)],
              ),
            ),
          ),
          
          Center(
            child: MatrixGlassCard(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    padding: const EdgeInsets.all(24),
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: Colors.tealAccent.withAlpha(25),
                      boxShadow: [
                         BoxShadow(
                          color: Colors.tealAccent.withAlpha(50),
                          blurRadius: 20,
                          spreadRadius: 2
                         )
                      ]
                    ),
                    child: const Icon(Icons.security, size: 60, color: Colors.tealAccent),
                  ),
                  const SizedBox(height: 30),
                  const Text('Matrix Quant Core', 
                    style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold, letterSpacing: 1.2, color: Colors.white)),
                  const SizedBox(height: 10),
                  const Text('System Locked', style: TextStyle(fontSize: 14, color: Colors.grey)),
                  const SizedBox(height: 40),
                  if(!_isAuthenticating) 
                    ElevatedButton.icon(
                      onPressed: _authenticate,
                      icon: const Icon(Icons.fingerprint, size: 28),
                      label: const Text('Authenticate to Enter', style: TextStyle(fontWeight: FontWeight.w600)),
                      style: ElevatedButton.styleFrom(
                        padding: const EdgeInsets.symmetric(horizontal: 40, vertical: 18),
                        backgroundColor: Colors.teal,
                        foregroundColor: Colors.white,
                        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(15)),
                        elevation: 5,
                        shadowColor: Colors.tealAccent.withAlpha(100)
                      ),
                    ),
                  if(_isAuthenticating)
                     const CircularProgressIndicator(color: Colors.tealAccent),
                ],
              ),
            ),
          ),
        ],
      )
    );
  }
}

class MatrixGlassCard extends StatelessWidget {
  final Widget child;
  const MatrixGlassCard({Key? key, required this.child}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(24),
      child: BackdropFilter(
        filter: ImageFilter.blur(sigmaX: 15.0, sigmaY: 15.0),
        child: Container(
          padding: const EdgeInsets.all(32),
          decoration: BoxDecoration(
            color: Colors.white.withAlpha(15),
            borderRadius: BorderRadius.circular(24),
            border: Border.all(color: Colors.white.withAlpha(25)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withAlpha(50),
                blurRadius: 30,
              )
            ]
          ),
          child: child,
        ),
      ),
    );
  }
}

class DashboardScreen extends StatefulWidget {
  const DashboardScreen({Key? key}) : super(key: key);

  @override
  State<DashboardScreen> createState() => _DashboardScreenState();
}

class _DashboardScreenState extends State<DashboardScreen> {
  String _greeting = "Loading from Rust Engine...";

  @override
  void initState() {
    super.initState();
    _callRust();
  }

  Future<void> _callRust() async {
    try {
      final response = await greet(name: "Matrix Quant Admin");
      setState(() {
        _greeting = response;
      });
    } catch (e) {
      setState(() {
        _greeting = "Rust Engine Error: \$e";
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      extendBodyBehindAppBar: true,
      appBar: AppBar(
        title: const Text('Matrix Quant Core', style: TextStyle(fontWeight: FontWeight.bold, letterSpacing: 1.2)),
        backgroundColor: Colors.transparent,
        elevation: 0,
        actions: [
          IconButton(
            icon: const Icon(Icons.settings, color: Colors.tealAccent),
            onPressed: () {},
          )
        ],
      ),
      body: Stack(
        children: [
          // Background Gradient
          Container(
            decoration: const BoxDecoration(
              gradient: RadialGradient(
                center: Alignment(0.5, -0.8),
                radius: 1.8,
                colors: [Color(0xFF1E293B), Color(0xFF0F172A)],
              ),
            ),
          ),
          
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(24.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const SizedBox(height: 20),
                  MatrixGlassCard(
                    child: Column(
                      children: [
                        Container(
                          padding: const EdgeInsets.all(20),
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: Colors.tealAccent.withAlpha(25),
                            boxShadow: [
                               BoxShadow(
                                color: Colors.tealAccent.withAlpha(50),
                                blurRadius: 20,
                                spreadRadius: 2
                               )
                            ]
                          ),
                          child: const Icon(
                            Icons.analytics_rounded,
                            size: 60,
                            color: Colors.tealAccent,
                          ),
                        ),
                        const SizedBox(height: 25),
                        const Text(
                          'Engine Status',
                          style: TextStyle(fontSize: 16, color: Colors.grey, letterSpacing: 1.1),
                        ),
                        const SizedBox(height: 10),
                        Text(
                          _greeting,
                          textAlign: TextAlign.center,
                          style: const TextStyle(fontSize: 20, fontWeight: FontWeight.bold, color: Colors.white),
                        ),
                      ],
                    ),
                  ),
                  
                  const SizedBox(height: 30),
                  
                  // Mock Grid for future data
                  Expanded(
                    child: GridView.count(
                      crossAxisCount: 2,
                      crossAxisSpacing: 16,
                      mainAxisSpacing: 16,
                      childAspectRatio: 1.1,
                      children: [
                        _buildStatCard('Active Pairs', '524', Icons.stacked_line_chart),
                        _buildStatCard('24h Volume', '€1.2M', Icons.euro_symbol),
                        _buildStatCard('Open Config', 'Aggressive', Icons.tune),
                        _buildStatCard('Kill Switch', 'Armed', Icons.warning_amber_rounded, iconColor: Colors.orangeAccent),
                      ],
                    )
                  )
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
  
  Widget _buildStatCard(String title, String value, IconData icon, {Color iconColor = Colors.tealAccent}) {
    return MatrixGlassCard(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(icon, color: iconColor, size: 32),
          const SizedBox(height: 12),
          Text(value, style: const TextStyle(fontSize: 24, fontWeight: FontWeight.bold, color: Colors.white)),
          const SizedBox(height: 4),
          Text(title, style: const TextStyle(fontSize: 12, color: Colors.grey)),
        ],
      )
    );
  }
}
