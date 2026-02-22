"""
LLM Intelligence Module for Trading Bot
Uses Gemini/Claude API to make intelligent exit decisions
"""

import os
from typing import Dict, Optional, Literal
import google.generativeai as genai
from anthropic import Anthropic


class LLMTradeAdvisor:
    """
    Intelligent exit advisor using Gemini or Claude
    """
    
    def __init__(self, provider: Literal["gemini", "claude"] = "gemini"):
        """
        Initialize LLM advisor
        
        Args:
            provider: "gemini" or "claude"
        """
        self.provider = provider
        
        if provider == "gemini":
            api_key = os.getenv("GEMINI_API_KEY")
            if not api_key:
                raise ValueError("GEMINI_API_KEY environment variable not set")
            genai.configure(api_key=api_key)
            self.model = genai.GenerativeModel('gemini-2.0-flash-exp')
        
        elif provider == "claude":
            api_key = os.getenv("ANTHROPIC_API_KEY")
            if not api_key:
                raise ValueError("ANTHROPIC_API_KEY environment variable not set")
            self.client = Anthropic(api_key=api_key)
        
        else:
            raise ValueError(f"Unknown provider: {provider}")
    
    def should_exit_trade(
        self, 
        pair: str,
        entry_price: float,
        current_price: float,
        current_profit_pct: float,
        time_in_trade_minutes: int,
        volume_24h_change_pct: float,
        rsi: float,
        macd_histogram: float
    ) -> Dict[str, any]:
        """
        Analyze if we should exit the trade now
        
        Returns:
            {
                "action": "HOLD" or "EXIT",
                "confidence": 0.0-1.0,
                "reasoning": str,
                "suggested_exit_pct": float (optional)
            }
        """
        
        prompt = f"""You are a cryptocurrency trading expert. Analyze this trade and decide if we should EXIT now or HOLD for more profit.

**Trade Details:**
- Pair: {pair}
- Entry Price: €{entry_price:.6f}
- Current Price: €{current_price:.6f}
- Current Profit: {current_profit_pct:.2f}%
- Time in Trade: {time_in_trade_minutes} minutes
- 24h Volume Change: {volume_24h_change_pct:.2f}%
- RSI: {rsi:.1f}
- MACD Histogram: {macd_histogram:.4f}

**Context:**
- We need minimum 5% profit to cover 0.3% trading fees and make profit
- Current trailing stop will exit at {current_profit_pct - 2.5:.2f}%
- This is a scalping strategy (short-term, < 1 hour)

**Your Task:**
Analyze the momentum and trend strength. Should we:
1. **EXIT NOW** - if trend is weakening or profit is good enough
2. **HOLD** - if strong upward momentum continues

Respond in this EXACT format:
ACTION: [HOLD or EXIT]
CONFIDENCE: [0.0-1.0]
REASONING: [one sentence why]
SUGGESTED_EXIT_PCT: [percentage, e.g., 6.5]
"""
        
        try:
            if self.provider == "gemini":
                response = self.model.generate_content(prompt)
                analysis_text = response.text
            else:  # claude
                message = self.client.messages.create(
                    model="claude-3-5-sonnet-20241022",
                    max_tokens=300,
                    messages=[{"role": "user", "content": prompt}]
                )
                analysis_text = message.content[0].text
            
            # Parse response
            action = "HOLD"
            confidence = 0.5
            reasoning = "Unable to parse LLM response"
            suggested_exit = None
            
            for line in analysis_text.split('\n'):
                line = line.strip()
                if line.startswith("ACTION:"):
                    action = "EXIT" if "EXIT" in line.upper() else "HOLD"
                elif line.startswith("CONFIDENCE:"):
                    try:
                        confidence = float(line.split(":")[1].strip())
                    except:
                        confidence = 0.5
                elif line.startswith("REASONING:"):
                    reasoning = line.split(":", 1)[1].strip()
                elif line.startswith("SUGGESTED_EXIT_PCT:"):
                    try:
                        suggested_exit = float(line.split(":")[1].strip().replace("%", ""))
                    except:
                        pass
            
            return {
                "action": action,
                "confidence": confidence,
                "reasoning": reasoning,
                "suggested_exit_pct": suggested_exit,
                "raw_response": analysis_text
            }
        
        except Exception as e:
            # Fallback to HOLD on error
            return {
                "action": "HOLD",
                "confidence": 0.0,
                "reasoning": f"LLM Error: {str(e)}",
                "suggested_exit_pct": None
            }
    
    def analyze_entry_opportunity(
        self,
        pair: str,
        current_price: float,
        rsi: float,
        volume_spike_pct: float,
        price_near_bb_lower_pct: float
    ) -> Dict[str, any]:
        """
        Analyze if this is a good entry opportunity
        
        Returns:
            {
                "action": "BUY" or "WAIT",
                "confidence": 0.0-1.0,
                "reasoning": str
            }
        """
        
        prompt = f"""Cryptocurrency entry analysis for {pair}:

**Market Data:**
- Current Price: €{current_price:.6f}
- RSI: {rsi:.1f} (oversold < 30, overbought > 70)
- Volume Spike: {volume_spike_pct:.1f}% above average
- Distance from Lower BB: {price_near_bb_lower_pct:.1f}%

Is this a good entry? We target 5%+ profit.

Respond:
ACTION: [BUY or WAIT]
CONFIDENCE: [0.0-1.0]
REASONING: [one sentence]
"""
        
        try:
            if self.provider == "gemini":
                response = self.model.generate_content(prompt)
                analysis_text = response.text
            else:
                message = self.client.messages.create(
                    model="claude-3-5-sonnet-20241022",
                    max_tokens=200,
                    messages=[{"role": "user", "content": prompt}]
                )
                analysis_text = message.content[0].text
            
            action = "WAIT"
            confidence = 0.5
            reasoning = "Unable to parse"
            
            for line in analysis_text.split('\n'):
                line = line.strip()
                if line.startswith("ACTION:"):
                    action = "BUY" if "BUY" in line.upper() else "WAIT"
                elif line.startswith("CONFIDENCE:"):
                    try:
                        confidence = float(line.split(":")[1].strip())
                    except:
                        pass
                elif line.startswith("REASONING:"):
                    reasoning = line.split(":", 1)[1].strip()
            
            return {
                "action": action,
                "confidence": confidence,
                "reasoning": reasoning
            }
        
        except Exception as e:
            return {
                "action": "WAIT",
                "confidence": 0.0,
                "reasoning": f"Error: {str(e)}"
            }


# Test function
if __name__ == "__main__":
    # Test with Gemini
    advisor = LLMTradeAdvisor(provider="gemini")
    
    result = advisor.should_exit_trade(
        pair="BTC/EUR",
        entry_price=50000.0,
        current_price=52500.0,
        current_profit_pct=5.0,
        time_in_trade_minutes=15,
        volume_24h_change_pct=25.0,
        rsi=68.0,
        macd_histogram=0.0012
    )
    
    print("LLM Decision:", result)
