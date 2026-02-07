---
stepsCompleted: [1, 2, 3, 4, 5]
inputDocuments: ["brainstorming/brainstorming-session-2026-02-03.md"]
date: 2026-02-03
author: Guy
---

# Product Brief: naic

## Executive Summary

**naic** is an open-source investment analysis platform designed to bring the disciplined NAIC (National Association of Investors Corporation) methodology into the modern, global era. By automating the extraction of 10-year historical financial data for international markets (beginning with Swiss and German tickers), **naic** eliminates the manual entry barrier that currently discourages analysts from researching non-US growth stocks. It combines tradition with technology, providing a "One-Click History" experience that empowers individual hobbyists to apply institutional-grade analysis to any company, anywhere.

---

## Core Vision

### Problem Statement

There is a lack of accessible, open-source tools that implement the NAIC Stock Selection Guide (SSG) for international companies. Existing solutions are either proprietary, US-centric, or require tedious manual entry of decade-long historical data, making global stock analysis extremely high-friction for individual investors.

### Problem Impact

Because manual data entry is so labor-intensive (requiring 10 years of sales, earnings, and price history), many hobbyist investors give up on applying the NAIC methodology to international opportunities. This leads to missed investment opportunities outside the US or the abandonment of a proven, disciplined analysis framework in favor of "gut feel" or incomplete data.

### Why Existing Solutions Fall Short

Current NAIC software is often a closed ecosystem that prioritizes US exchanges. For international stocks, these tools frequently lack automated data feeds, forcing users to manually source and transcribe data from annual reports or secondary websites. This process is time-consuming, error-prone, and unsustainable for most retail investors.

### Proposed Solution

An open-source, data-driven application that automates the NAIC SSG workflow for global markets. Users can enter an international ticker (starting with CH and DE) and instantly receive a populated 10-year historical chart, pre-calculated quality ratios, and relevant competitor data. This transforms the analysis from a "data entry chore" into a "decision-making experience."

### Key Differentiators

- **True Global Focus**: Native support for Swiss, German, and other non-US markets, handling multi-currency and international reporting standards.
- **"One-Click" Automation**: Direct API integration to populate a full decade of SSG-ready data instantly.
- **Open Source Integrity**: A transparent, community-driven platform that ensures the analysis logic is verifiable and accessible to everyone.
- **Integrated Peer Data**: Automatically pulls sector/competitor data required for the "benchmarking" sections of the SSG, a feature often missing in manual workarounds.

---

## Target Users

### Primary Users

#### The Hobbyist Analyst (Beginner to Advanced)

- **Profile**: Individual investors who range from beginners learning the NAIC methodology to experienced veterans who analyze 5–20 stocks per week.

- **Context**: They follow a disciplined growth-investing approach and maintain a watchlist of international stocks, with a strong focus on Swiss and German markets.
- **Pain Points**: The extreme friction of manual data entry, accounting for stock splits, managing different currencies, and calculating historical P/E ranges from raw reports.
- **Motivation**: To apply high-quality fundamental analysis to a large volume of opportunities without the process becoming a full-time "data entry" job.

### Secondary Users

#### Investment Club Members

- **Profile**: Members of local or online investment clubs who share reports and analyze stocks collaboratively.

- **Need**: They benefit from **naic's** ability to generate clear, standardized reports that can be used for group discussions and decision-making.

### User Journey

- **Discovery**: An investor looking for "NAIC for Swiss stocks" or "Open Source SSG tool" finds the project on GitHub or through investing forums.
- **Onboarding**: The user opens the app and is greeted by a clean "Zen" search bar. No complex setup is required to see the first analysis.
- **Core Usage**: The user enters a ticker (e.g., `NESN.SW` or `SAP.DE`).
- **The Success ("Aha!") Moment**: Instantly, a 10-year logarithmic chart appears, populated with sales, earnings, and price history. All the "chores" (splits, currency normalization) are handled. The user's focus shifts immediately from *transcribing* data to *interpreting* the company's growth story.
- **Long-term Value**: **naic** becomes the primary tool for quarterly watchlist reviews, enabling the user to maintain a much broader and more geographically diverse portfolio with significantly less effort.

---

## Success Metrics

### User Success Metrics

- **Time-to-Analysis (Beginners)**: A beginner user is able to complete a full NAIC analysis (from ticker entry to finalized SSG) in **30 minutes or less**.
- **Analysis Confidence**: Users express high confidence in the accuracy of the automated data, verified by the lack of "manual override" needs for core 10-year historicals.
- **Comparison Ease**: Users successfully perform side-by-side comparisons of multiple stocks (e.g., Swiss vs. German peers) as part of their regular workflow.
- **Risk Oversight**: Users actively use the platform to monitor and manage portfolio-wide risks (e.g., sector concentration or growth slowdowns).

### Business Objectives

- **Growth (3-6 Months)**: Establish **naic** as a recognized open-source alternative for international NAIC investors, measured by GitHub repository interest and community engagement.
- **Strategic Position**: Successfully bridge the gap between US-centric proprietary software and the needs of the European (CH/DE) investment community.
- **Functional Excellence**: Provide a 100% reliable "One-Click" data pipeline for at least three major international exchanges (SMI, DAX, and a major US exchange).

### Key Performance Indicators

- **Adoption**: Number of application downloads/releases via GitHub.
- **Engagement**: Number of unique stocks analyzed per active user (target: 5–20 per week for primary segments).
- **Performance**: Average system response time for a "One-Click" 10-year historical data population (target: < 5 seconds).

---

## MVP Scope

### Core Features

- **"One-Click" Data Engine**: High-reliability automated 10-year historical data population for US, Swiss (SWX), and German (DAX) markets.
- **Classic SSG Visualization**: A web-native logarithmic charting engine that renders Sales, Earnings per Share, and Price trends according to NAIC standards.
- **Quality Ratio Dashboard**: Automated calculation and display of Pre-tax Profit on Sales, Return on Equity, and Historical P/E ranges.
- **Side-by-Side Comparison**: Ability to overlay or compare two analyzed stocks for peer benchmarking.
- **Open Source Foundation**: Transparent, auditable code base with community-friendly licensing.

### Out of Scope for MVP

- **Portfolio Tracking**: Real-time management of holdings and performance (deferred to Phase 2).
- **Kinetic Charting**: Manual drag-and-drop trend line manipulation (deferred to Phase 3).
- **Annual Report OCR**: PDF parsing for missing historical data (deferred to Phase 4).
- **International Markets Beyond US/CH/DE**: French, UK, or Asian market support.

### MVP Success Criteria

- **30-Minute Threshold**: A beginner can finalize a quality analysis on a Swiss stock in under 30 minutes.
- **Data Parity**: Automated data matches the accuracy of official company reports for at least 95% of tested large and mid-cap tickers.
- **Zero-Manual Entry**: Users can generate a complete visual chart without typing a single financial figure manually.

### Future Vision

- **Phase 2: Portfolio Lifecycle**: Integrated portfolio tracking with rebalancing alerts and "Dividend Dashboards."
- **Phase 3: The Tactile Analyst**: "Kinetic" charting implementation for interactive pattern matching and projection "shadowing."
- **Phase 4: The Data Oracle**: AI-powered OCR to ingest data from any PDF annual report, solving the "data gap" for obscure international or OTC companies.
- **Beyond**: Expansion to all major global exchanges and a community-driven library of "Gold Standard" historical SSGs.
