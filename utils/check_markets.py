#!/usr/bin/env python3
"""Show ALL available markets on One Trading"""

import ccxt

exchange = ccxt.onetrading({
    'apiKey': 'eyJvcmciOiJiaXRwYW5kYS1nZSIsImFsZyI6IlJTMjU2Iiwia2lkIjoiZXhjaGFuZ2UtbGl2ZSJ9.eyJhdWQiOlsiaHR0cHM6Ly9hcGkuZXhjaGFuZ2UuYml0cGFuZGEuY29tIiwid3NzOi8vc3RyZWFtcy5leGNoYW5nZS5iaXRwYW5kYS5jb20iXSwic3ViIjoiYWNjOjU3YjBkZjk1LTExYWItNDQ2OS1hMWI4LTE2MWQ0YjA4MWMyYyIsInNjcCI6WyJSRUFEX09OTFkiLCJUUkFERSJdLCJuYmYiOjE3NzA2NjUyODEsImFjaCI6ImJwYzo1N2IwZGY5NS0xMWFiLTQ0NjktYTFiOC0xNjFkNGIwODFjMmMiLCJpc3MiOiJodHRwczovL2FwaS5leGNoYW5nZS5iaXRwYW5kYS5jb20vb2F1dGgyIiwiaXBzIjpbXSwiaWF0IjoxNzcwNjY1MjgxLCJqdGkiOiI4YjA5ZjI0Ny02ZTkxLTQyNmEtYmVlNC05YWNkMDZiNzUzMzQifQ.XycKxWUEWA_1R3qilmJOaFGVnexHvzJ5BQdK7dEiSLbyxl2RYt26IjWtlQfeJJTEvliOsCWygmdvB0hdUMW7HLvEaWEXPZplAavmYyPu-Gbmrvx665xMYKfhsVYCd3UABFLh_Ab8Bwo4bRH8bWeNb8DxniiNyRSDZn65zlaj7LZrAspGqTxZd2tYyPRHYuKGxdqD2sGX9kqxjz79PFz-apYpPX701DMx6n74KFk0VcL7_ijetTjadavr1AkunJZG1dkIzYmSm5HeHxVkGu_W21wFjD4kE48SMZ0YxTFtOdborOtuxpMQax2-Kcv2p4gig_8xaN-XekMTM396P-ZCz1l1Ni5Fa12cOa2wvg8kaW57cM3hznuHZ-CcAEPLwMZw0f1weicS9fWP2sJtpKLXICIeOo4iNuMPB2rK3s9yVhhK0W-ZFMg0FeR6j3LkKEICYiiqJ_OslYUlaW2ef-3j2APAfOjNYpOKnZ-trAnU9XycXEKDVKbbSIdMHDs9fysRHWgidVd9u9mY8X42ms9CyOXJcNTkNTuYke1SRsy0fpS0WRHwhdq3xWqsDq_oaZN0SvOvnCubZCRm1pp0e5V2PL48aUfhG8V5HYtwnmWPUjhdxa7cqsYsAhZ9zkMfTIjZr05jexz2VHOxy4V5aAGRH2liV8YHfmbXURxJ_OKZBuI',
    'secret': '',
})

markets = exchange.load_markets()

# Group by quote currency
by_quote = {}
for symbol, market in markets.items():
    if market['active']:
        quote = market['quote']
        if quote not in by_quote:
            by_quote[quote] = []
        by_quote[quote].append(symbol)

print("📊 VERFÜGBARE MÄRKTE AUF ONE TRADING:\n")

for quote, pairs in sorted(by_quote.items()):
    print(f"\n💱 {quote}-Paare ({len(pairs)} aktiv):")
    for pair in sorted(pairs):
        print(f"   {pair}")

print(f"\n\n✅ Total: {sum(len(p) for p in by_quote.values())} aktive Märkte")
print(f"📈 Quote-Währungen: {', '.join(sorted(by_quote.keys()))}")
