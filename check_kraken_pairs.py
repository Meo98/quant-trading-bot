import ccxt
print("Connecting to Kraken for Market Data...")
try:
    kraken = ccxt.kraken()
    markets = kraken.load_markets()
    
    eur_pairs = [s for s in markets if s.endswith('/EUR')]
    usd_pairs = [s for s in markets if s.endswith('/USD')]
    chf_pairs = [s for s in markets if s.endswith('/CHF')]
    
    print(f"\n📊 AVAILABLE CURRENCY PAIRS ON KRAKEN:")
    print(f"  🇪🇺 EUR Pairs: {len(eur_pairs)} (e.g. BTC/EUR, ETH/EUR, SOL/EUR...)")
    print(f"  🇺🇸 USD Pairs: {len(usd_pairs)} (e.g. BTC/USD...)")
    print(f"  🇨🇭 CHF Pairs: {len(chf_pairs)} (Current Balance)")
    
    print("\n🔍 CHF Pairs List:")
    for p in chf_pairs:
        print(f"  - {p}")
        
    print("\n💡 ADVICE:")
    if len(chf_pairs) < 20:
        print("  Trading ONLY CHF limits you to very few coins.")
        print("  Recommended: Swap CHF to EUR to access hundreds of altcoins!")
except Exception as e:
    print("Error:", e)
