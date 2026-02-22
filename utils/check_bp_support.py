import ccxt
print("CCXT Version:", ccxt.__version__)
try:
    bp = ccxt.bitpanda()
    print("Bitpanda (Broker?) URLs:", bp.urls['api'])
    
    ot = ccxt.onetrading()
    print("OneTrading URLs:", ot.urls['api'])
    
    # Check if 'bitpanda' API is distinct from 'onetrading'
    if bp.urls['api']['public'] == ot.urls['api']['public']:
        print("RESULT: 'bitpanda' in CCXT refers to One Trading (same API).")
        print("There is NO dedicated CCXT driver for the Bitpanda Broker App.")
    else:
        print("RESULT: 'bitpanda' is distinct. Might be the Broker API?")
except Exception as e:
    print("Error:", e)
