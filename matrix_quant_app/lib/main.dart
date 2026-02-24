import 'package:flutter/material.dart';
import 'package:matrix_quant_app/src/rust/frb_generated.dart';
import 'package:matrix_quant_app/src/rust/api/simple.dart';
import 'package:local_auth/local_auth.dart';
import 'package:flutter/services.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const MatrixQuantApp());
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
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.lock_outline, size: 80, color: Colors.teal),
            const SizedBox(height: 20),
            const Text('Security Status: Locked', style: TextStyle(fontSize: 16)),
            const SizedBox(height: 40),
            if(!_isAuthenticating) 
              ElevatedButton.icon(
                onPressed: _authenticate,
                icon: const Icon(Icons.fingerprint),
                label: const Text('Unlock with Biometrics'),
                style: ElevatedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(horizontal: 40, vertical: 15),
                  backgroundColor: Colors.teal,
                  foregroundColor: Colors.white
                ),
              ),
          ],
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
      appBar: AppBar(
        title: const Text('Matrix Quant Core', style: TextStyle(fontWeight: FontWeight.bold)),
        backgroundColor: Colors.transparent,
        elevation: 0,
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
             Container(
              padding: const EdgeInsets.all(20),
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: Colors.teal.withAlpha(25),
              ),
              child: const Icon(
                Icons.analytics_rounded,
                size: 80,
                color: Colors.tealAccent,
              ),
            ),
            const SizedBox(height: 30),
            const Text(
              'Rust Engine Status:',
              style: TextStyle(fontSize: 16, color: Colors.grey),
            ),
            const SizedBox(height: 10),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
              decoration: BoxDecoration(
                color: Colors.white.withAlpha(12),
                borderRadius: BorderRadius.circular(15),
                border: Border.all(color: Colors.teal.withAlpha(76)),
                boxShadow: [
                   BoxShadow(
                    color: Colors.teal.withAlpha(12),
                    blurRadius: 10,
                    spreadRadius: 2
                   )
                ]
              ),
              child: Text(
                _greeting,
                style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold, color: Colors.white),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
