#!/usr/bin/env python3

import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
import sys
import os
import numpy as np

def plot_sweep(csv_file, x_col, title):
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    if df.empty:
        print(f"⚠️  CSV file is empty: {csv_file}")
        return

    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle(title, fontsize=16)

    axes[0, 0].plot(df[x_col], df['precondition_failure_rate'] * 100, marker='o')
    axes[0, 0].axhline(y=10, color='r', linestyle='--', label='10% threshold')
    axes[0, 0].set_xlabel(x_col)
    axes[0, 0].set_ylabel('Precondition Failure Rate (%)')
    axes[0, 0].legend()
    axes[0, 0].grid(True)

    axes[0, 1].plot(df[x_col], df['write_tps'], marker='o', color='green')
    axes[0, 1].set_xlabel(x_col)
    axes[0, 1].set_ylabel('Write TPS')
    axes[0, 1].grid(True)

    axes[1, 0].plot(df[x_col], df['write_p50_ms'], marker='o', label='p50')
    axes[1, 0].plot(df[x_col], df['write_p95_ms'], marker='s', label='p95')
    axes[1, 0].plot(df[x_col], df['write_p99_ms'], marker='^', label='p99')
    axes[1, 0].set_xlabel(x_col)
    axes[1, 0].set_ylabel('Write Latency (ms)')
    axes[1, 0].legend()
    axes[1, 0].grid(True)

    axes[1, 1].plot(df[x_col], df['precond_p50_ms'], marker='o', label='p50')
    axes[1, 1].plot(df[x_col], df['precond_p99_ms'], marker='^', label='p99')
    axes[1, 1].set_xlabel(x_col)
    axes[1, 1].set_ylabel('Precondition Failure Latency (ms)')
    axes[1, 1].legend()
    axes[1, 1].grid(True)

    plt.tight_layout()
    output_file = f'{csv_file[:-4]}.png'
    plt.savefig(output_file, dpi=300)
    print(f"✅ Plot saved to {output_file}")
    plt.close()

def plot_comprehensive_sweep(csv_file):
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    if df.empty:
        print(f"⚠️  CSV file is empty: {csv_file}")
        return

    print(f"Loaded {len(df)} configurations from {csv_file}")

    fig = plt.figure(figsize=(18, 12))
    gs = fig.add_gridspec(3, 3, hspace=0.3, wspace=0.3)
    fig.suptitle('Comprehensive Configuration Sweep Results', fontsize=16, fontweight='bold')

    ax1 = fig.add_subplot(gs[0, 0])
    for num_writers in df['num_writers'].unique():
        subset = df[df['num_writers'] == num_writers]
        ax1.scatter(subset['writer_rate'], subset['precondition_failure_rate'] * 100,
                   label=f'{num_writers} writers', alpha=0.7, s=30)
    ax1.axhline(y=10, color='r', linestyle='--', linewidth=2, label='10% threshold')
    ax1.set_xlabel('Writer Rate (ops/sec)', fontsize=10)
    ax1.set_ylabel('Precondition Failure Rate (%)', fontsize=10)
    ax1.set_title('Failure Rate vs Writer Rate', fontsize=11, fontweight='bold')
    ax1.legend(fontsize=8)
    ax1.grid(True, alpha=0.3)

    ax2 = fig.add_subplot(gs[0, 1])
    for overlap in df['key_overlap_ratio'].unique():
        subset = df[df['key_overlap_ratio'] == overlap]
        ax2.scatter(subset['num_writers'], subset['write_tps'],
                   label=f'Overlap {overlap:.1f}', alpha=0.7, s=30)
    ax2.set_xlabel('Number of Writers', fontsize=10)
    ax2.set_ylabel('Write TPS', fontsize=10)
    ax2.set_title('Write TPS vs Number of Writers', fontsize=11, fontweight='bold')
    ax2.legend(fontsize=8)
    ax2.grid(True, alpha=0.3)

    ax3 = fig.add_subplot(gs[0, 2])
    pivot_data = df.pivot_table(
        values='precondition_failure_rate',
        index='num_writers',
        columns='key_overlap_ratio',
        aggfunc='mean'
    ) * 100
    im = ax3.imshow(pivot_data, cmap='RdYlGn_r', aspect='auto', vmin=0, vmax=100)
    ax3.set_xticks(range(len(pivot_data.columns)))
    ax3.set_yticks(range(len(pivot_data.index)))
    ax3.set_xticklabels([f'{x:.1f}' for x in pivot_data.columns], fontsize=9)
    ax3.set_yticklabels(pivot_data.index, fontsize=9)
    ax3.set_xlabel('Key Overlap Ratio', fontsize=10)
    ax3.set_ylabel('Number of Writers', fontsize=10)
    ax3.set_title('Failure Rate Heatmap (%)', fontsize=11, fontweight='bold')
    plt.colorbar(im, ax=ax3, label='Failure Rate %')

    ax4 = fig.add_subplot(gs[1, :])
    df_sorted = df.sort_values('precondition_failure_rate')
    top_10 = df_sorted.head(10)
    colors = plt.cm.viridis(np.linspace(0, 1, 10))
    bars = ax4.barh(range(10), top_10['precondition_failure_rate'] * 100, color=colors)
    ax4.set_yticks(range(10))
    ax4.set_yticklabels(top_10['config_label'], fontsize=8)
    ax4.set_xlabel('Precondition Failure Rate (%)', fontsize=10)
    ax4.set_title('Top 10 Best Configurations (Lowest Failure Rate)', fontsize=11, fontweight='bold')
    ax4.grid(True, axis='x', alpha=0.3)
    ax4.invert_yaxis()

    ax5 = fig.add_subplot(gs[2, 0])
    for reader_rate in df['reader_rate'].unique():
        subset = df[df['reader_rate'] == reader_rate]
        ax5.scatter(subset['key_overlap_ratio'], subset['precondition_failure_rate'] * 100,
                   label=f'Reader rate {reader_rate:.0f}', alpha=0.7, s=30)
    ax5.set_xlabel('Key Overlap Ratio', fontsize=10)
    ax5.set_ylabel('Precondition Failure Rate (%)', fontsize=10)
    ax5.set_title('Failure Rate vs Overlap', fontsize=11, fontweight='bold')
    ax5.legend(fontsize=8)
    ax5.grid(True, alpha=0.3)

    ax6 = fig.add_subplot(gs[2, 1])
    ax6.scatter(df['precondition_failure_rate'] * 100, df['write_tps'],
               c=df['num_writers'], cmap='viridis', alpha=0.6, s=30)
    ax6.set_xlabel('Precondition Failure Rate (%)', fontsize=10)
    ax6.set_ylabel('Write TPS', fontsize=10)
    ax6.set_title('TPS vs Failure Rate', fontsize=11, fontweight='bold')
    cbar = plt.colorbar(ax6.collections[0], ax=ax6, label='# Writers')
    ax6.grid(True, alpha=0.3)

    ax7 = fig.add_subplot(gs[2, 2])
    ax7.scatter(df['write_p99_ms'], df['precondition_failure_rate'] * 100,
               c=df['writer_rate'], cmap='plasma', alpha=0.6, s=30)
    ax7.set_xlabel('Write P99 Latency (ms)', fontsize=10)
    ax7.set_ylabel('Precondition Failure Rate (%)', fontsize=10)
    ax7.set_title('Latency vs Failure Rate', fontsize=11, fontweight='bold')
    cbar = plt.colorbar(ax7.collections[0], ax=ax7, label='Writer Rate')
    ax7.grid(True, alpha=0.3)

    output_file = f'{csv_file[:-4]}.png'
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Comprehensive plot saved to {output_file}")
    plt.close()

def plot_chaos_sweep(csv_file):
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    if df.empty:
        print(f"⚠️  CSV file is empty: {csv_file}")
        return

    scenario_labels = [
        'Baseline',
        'Net Delay 100ms',
        'Net Delay 200ms',
        'Net Delay 500ms',
        'Net Block 10s×3',
        'CPU 4T@80%',
        'Combined'
    ]

    if len(df) != len(scenario_labels):
        print(f"⚠️  Expected {len(scenario_labels)} scenarios, got {len(df)}")

    df['scenario'] = scenario_labels[:len(df)]
    df['write_success_rate'] = 100.0 - (df['precondition_failure_rate'] * 100.0)

    print(f"Loaded {len(df)} chaos scenarios from {csv_file}")

    fig = plt.figure(figsize=(16, 12))
    gs = fig.add_gridspec(2, 2, hspace=0.3, wspace=0.3)
    fig.suptitle('Chaos Engineering Test Results', fontsize=16, fontweight='bold')

    # Subplot 1: Write Latency (top-left)
    ax1 = fig.add_subplot(gs[0, 0])
    x = range(len(df))
    ax1.plot(x, df['write_p50_ms'], marker='o', label='p50', linewidth=2, markersize=8, color='blue')
    ax1.plot(x, df['write_p95_ms'], marker='s', label='p95', linewidth=2, markersize=8, color='darkblue')
    ax1.plot(x, df['write_p99_ms'], marker='^', label='p99', linewidth=2, markersize=8, color='navy')
    ax1.set_xticks(x)
    ax1.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=10)
    ax1.set_ylabel('Write Latency (ms)', fontsize=12)
    ax1.set_title('Write Latency Under Chaos', fontsize=13, fontweight='bold')
    ax1.legend(fontsize=10)
    ax1.grid(True, alpha=0.3)

    # Subplot 2: Read Latency (top-right)
    ax2 = fig.add_subplot(gs[0, 1])
    ax2.plot(x, df['read_p50_ms'], marker='o', label='p50', linewidth=2, markersize=8, color='green')
    ax2.plot(x, df['read_p99_ms'], marker='^', label='p99', linewidth=2, markersize=8, color='darkgreen')
    ax2.set_xticks(x)
    ax2.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=10)
    ax2.set_ylabel('Read Latency (ms)', fontsize=12)
    ax2.set_title('Read Latency Under Chaos', fontsize=13, fontweight='bold')
    ax2.legend(fontsize=10)
    ax2.grid(True, alpha=0.3)

    # Subplot 3: Write Success Rate (bottom-left)
    ax3 = fig.add_subplot(gs[1, 0])
    colors = ['green' if i == 0 else 'orange' if 'Net' in s else 'red' if 'CPU' in s else 'purple'
              for i, s in enumerate(df['scenario'])]
    bars = ax3.bar(x, df['write_success_rate'], color=colors, alpha=0.7)
    ax3.set_xticks(x)
    ax3.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=10)
    ax3.set_ylabel('Write Success Rate (%)', fontsize=12)
    ax3.set_title('Write Success Rate Under Chaos', fontsize=13, fontweight='bold')
    ax3.set_ylim([0, 105])
    ax3.grid(True, axis='y', alpha=0.3)

    # Subplot 4: Retry Effectiveness (bottom-right)
    ax4 = fig.add_subplot(gs[1, 1])
    width = 0.35
    ax4.bar([i - width/2 for i in x], df['retry_success_rate'] * 100, width,
            label='Retry Success Rate', color='green', alpha=0.7)
    ax4.bar([i + width/2 for i in x], df['retry_failure_rate'] * 100, width,
            label='Retry Failure Rate', color='red', alpha=0.7)
    ax4.set_xticks(x)
    ax4.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=10)
    ax4.set_ylabel('Rate (%)', fontsize=12)
    ax4.set_title('Retry Effectiveness', fontsize=13, fontweight='bold')
    ax4.legend(fontsize=10)
    ax4.grid(True, axis='y', alpha=0.3)

    plt.tight_layout()
    output_file = f'{csv_file[:-4]}.png'
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Chaos plot saved to {output_file}")
    plt.close()

def plot_latency_sweep(csv_file):
    """Plot latency trends across configurations with connected dots."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)
    df = df.sort_values(['num_writers', 'writer_rate'])
    df['config'] = df.apply(lambda row: f"W{int(row['num_writers'])}_R{row['writer_rate']:.2f}", axis=1)

    fig, ax = plt.subplots(figsize=(14, 8))
    x = range(len(df))

    # Plot writer and reader latencies
    ax.plot(x, df['write_p50_ms'], 'o-', color='blue', label='Writer p50', linewidth=2, markersize=8)
    ax.plot(x, df['write_p99_ms'], 'o--', color='blue', label='Writer p99', linewidth=2, markersize=8, alpha=0.7)
    ax.plot(x, df['read_p50_ms'], 's-', color='green', label='Reader p50', linewidth=2, markersize=8)
    ax.plot(x, df['read_p99_ms'], 's--', color='green', label='Reader p99', linewidth=2, markersize=8, alpha=0.7)

    ax.set_xticks(x)
    ax.set_xticklabels(df['config'].tolist(), rotation=45, ha='right')
    ax.set_xlabel('Configuration (Writers_WriterRate)', fontsize=12)
    ax.set_ylabel('Latency (ms)', fontsize=12)
    ax.set_title('Latency Trends Across Writer Configurations', fontsize=14, fontweight='bold')
    ax.legend(fontsize=11, loc='best')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '_latency.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Latency plot saved to {output_file}")
    plt.close()

def plot_comprehensive_v2_read_latency(csv_file):
    """Plot read latency (p50, p99) vs config - all reader configs + baseline."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    baseline_df = None
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        print(f"  📌 Including baseline data from test_baseline.csv")

    df = df.sort_values(['num_readers', 'reader_rate', 'num_writers', 'writer_rate'])
    df['config'] = df.apply(lambda row: f"R{int(row['num_readers'])}@{int(row['reader_rate'])}_W{int(row['num_writers'])}@{row['writer_rate']:.2f}", axis=1)

    fig, ax = plt.subplots(figsize=(20, 8))
    x = range(len(df))

    ax.plot(x, df['read_p50_ms'], 'o-', color='green', label='Reader p50', linewidth=2, markersize=6)
    ax.plot(x, df['read_p99_ms'], 's--', color='darkgreen', label='Reader p99', linewidth=2, markersize=6, alpha=0.7)

    if baseline_df is not None and len(baseline_df) > 0:
        baseline_p50 = baseline_df['read_p50_ms'].iloc[0]
        baseline_p99 = baseline_df['read_p99_ms'].iloc[0]
        ax.axhline(y=baseline_p50, color='green', linestyle=':', linewidth=2, label=f'Baseline p50 ({baseline_p50:.1f}ms)', alpha=0.8)
        ax.axhline(y=baseline_p99, color='darkgreen', linestyle=':', linewidth=2, label=f'Baseline p99 ({baseline_p99:.1f}ms)', alpha=0.8)

    ax.set_xticks(x[::max(1, len(x)//30)])
    ax.set_xticklabels(df['config'].tolist()[::max(1, len(x)//30)], rotation=90, ha='right', fontsize=8)
    ax.set_xlabel('Configuration (Readers@ReadRate_Writers@WriteRate)', fontsize=12)
    ax.set_ylabel('Read Latency (ms)', fontsize=12)
    ax.set_title('Read Latency vs Configuration (with Baseline Reference)', fontsize=14, fontweight='bold')
    ax.legend(fontsize=11, loc='best')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '_read_latency.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Read latency plot saved to {output_file}")
    plt.close()

def plot_comprehensive_v2_write_latency(csv_file):
    """Plot write latency (p50, p99) vs config - all configs + baseline."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    baseline_df = None
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        print(f"  📌 Including baseline data from test_baseline.csv")

    df = df.sort_values(['num_writers', 'writer_rate', 'num_readers', 'reader_rate'])
    df['config'] = df.apply(lambda row: f"W{int(row['num_writers'])}@{row['writer_rate']:.2f}_R{int(row['num_readers'])}@{int(row['reader_rate'])}", axis=1)

    fig, ax = plt.subplots(figsize=(20, 8))
    x = range(len(df))

    ax.plot(x, df['write_p50_ms'], 'o-', color='blue', label='Writer p50', linewidth=2, markersize=6)
    ax.plot(x, df['write_p99_ms'], 's--', color='darkblue', label='Writer p99', linewidth=2, markersize=6, alpha=0.7)

    if baseline_df is not None and len(baseline_df) > 0:
        baseline_p50 = baseline_df['write_p50_ms'].iloc[0]
        baseline_p99 = baseline_df['write_p99_ms'].iloc[0]
        ax.axhline(y=baseline_p50, color='blue', linestyle=':', linewidth=2, label=f'Baseline p50 ({baseline_p50:.1f}ms)', alpha=0.8)
        ax.axhline(y=baseline_p99, color='darkblue', linestyle=':', linewidth=2, label=f'Baseline p99 ({baseline_p99:.1f}ms)', alpha=0.8)

    ax.set_xticks(x[::max(1, len(x)//30)])
    ax.set_xticklabels(df['config'].tolist()[::max(1, len(x)//30)], rotation=90, ha='right', fontsize=8)
    ax.set_xlabel('Configuration (Writers@WriteRate_Readers@ReadRate)', fontsize=12)
    ax.set_ylabel('Write Latency (ms)', fontsize=12)
    ax.set_title('Write Latency vs Configuration (with Baseline Reference)', fontsize=14, fontweight='bold')
    ax.legend(fontsize=11, loc='best')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '_write_latency.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Write latency plot saved to {output_file}")
    plt.close()

def plot_comprehensive_v2_precondition_failure(csv_file):
    """Plot precondition failure rate vs writer config only (readers don't affect this) + baseline."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    baseline_df = None
    baseline_rate = 0.0
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        baseline_rate = baseline_df['precondition_failure_rate'].iloc[0]
        print(f"  📌 Including baseline data from test_baseline.csv (failure rate: {baseline_rate:.2f}%)")

    grouped = df.groupby(['num_writers', 'writer_rate']).agg({
        'precondition_failure_rate': 'mean'
    }).reset_index()

    grouped = grouped.sort_values(['num_writers', 'writer_rate'])
    grouped['config'] = grouped.apply(lambda row: f"W{int(row['num_writers'])}@{row['writer_rate']:.2f}", axis=1)

    fig, ax = plt.subplots(figsize=(14, 8))
    x = range(len(grouped))

    ax.plot(x, grouped['precondition_failure_rate'], 'o-', color='red', label='Precondition Failure Rate', linewidth=2, markersize=8)

    if baseline_df is not None:
        ax.axhline(y=baseline_rate, color='red', linestyle=':', linewidth=2, label=f'Baseline (W1@0.1): {baseline_rate:.2f}%', alpha=0.8)

    ax.set_xticks(x)
    ax.set_xticklabels(grouped['config'].tolist(), rotation=45, ha='right')
    ax.set_xlabel('Writer Configuration (Writers@WriteRate)', fontsize=12)
    ax.set_ylabel('Precondition Failure Rate (%)', fontsize=12)
    ax.set_title('Precondition Failure Rate vs Writer Configuration (with Baseline Reference)', fontsize=14, fontweight='bold')
    ax.legend(fontsize=11, loc='best')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '_precondition_failure.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Precondition failure plot saved to {output_file}")
    plt.close()

def plot_overlap_ratio_sweep(csv_file):
    """Plot precondition failure rate and retry success rate vs overlap ratio."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    df = pd.read_csv(csv_file)

    if df.empty:
        print(f"⚠️  CSV file is empty: {csv_file}")
        return

    print(f"Loaded {len(df)} configurations from {csv_file}")

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(14, 10))
    fig.suptitle('Overlap Ratio Sweep: Finding the 10% Failure Sweet Spot', fontsize=16, fontweight='bold')

    # Subplot 1: Precondition Failure Rate and Retry Success Rate
    x = df['key_overlap_ratio']
    ax1_twin = ax1.twinx()

    line1 = ax1.plot(x, df['precondition_failure_rate'] * 100, 'o-', color='red',
                     label='Precondition Failure Rate', linewidth=2, markersize=8)
    line2 = ax1_twin.plot(x, df['retry_success_rate'] * 100, 's-', color='green',
                          label='Retry Success Rate', linewidth=2, markersize=8)

    ax1.axhline(y=10, color='orange', linestyle='--', linewidth=2, label='10% Target', alpha=0.7)
    ax1.set_xlabel('Key Overlap Ratio', fontsize=12)
    ax1.set_ylabel('Precondition Failure Rate (%)', fontsize=12, color='red')
    ax1_twin.set_ylabel('Retry Success Rate (%)', fontsize=12, color='green')
    ax1.tick_params(axis='y', labelcolor='red')
    ax1_twin.tick_params(axis='y', labelcolor='green')
    ax1.set_title('Failure Rate and Retry Success vs Overlap Ratio', fontsize=14, fontweight='bold')

    lines = line1 + line2 + [plt.Line2D([0], [0], color='orange', linestyle='--', linewidth=2)]
    labels = ['Precondition Failure Rate', 'Retry Success Rate', '10% Target']
    ax1.legend(lines, labels, fontsize=10, loc='upper left')
    ax1.grid(True, alpha=0.3)

    # Subplot 2: Write TPS vs Overlap Ratio
    ax2.plot(x, df['write_tps'], 'o-', color='blue', label='Write TPS', linewidth=2, markersize=8)
    ax2.set_xlabel('Key Overlap Ratio', fontsize=12)
    ax2.set_ylabel('Write Throughput (TPS)', fontsize=12)
    ax2.set_title('Write Throughput vs Overlap Ratio', fontsize=14, fontweight='bold')
    ax2.legend(fontsize=10)
    ax2.grid(True, alpha=0.3)

    # Annotate sweet spot (closest to 10% failure rate)
    df['diff_from_10'] = abs(df['precondition_failure_rate'] * 100 - 10)
    sweet_spot_idx = df['diff_from_10'].idxmin()
    sweet_spot = df.loc[sweet_spot_idx]

    ax1.annotate('Sweet Spot!',
                xy=(sweet_spot['key_overlap_ratio'], sweet_spot['precondition_failure_rate'] * 100),
                xytext=(10, -30), textcoords='offset points',
                bbox=dict(boxstyle='round,pad=0.5', fc='yellow', alpha=0.7),
                arrowprops=dict(arrowstyle='->', connectionstyle='arc3,rad=0', color='black'),
                fontsize=10, fontweight='bold')

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Overlap ratio sweep plot saved to {output_file}")
    print(f"   Sweet spot: overlap_ratio={sweet_spot['key_overlap_ratio']:.2f}, "
          f"failure_rate={sweet_spot['precondition_failure_rate']*100:.2f}%")
    plt.close()

def plot_comprehensive_v2_all(csv_file):
    """Generate single PNG with all 4 comprehensive v2 plots as subplots."""
    if not os.path.exists(csv_file):
        print(f"⚠️  CSV file not found: {csv_file}")
        return

    print(f"\n📊 Generating comprehensive sweep plot from {csv_file}...")

    df = pd.read_csv(csv_file)

    # Load baseline data
    baseline_df = None
    if os.path.exists('test_baseline.csv'):
        baseline_df = pd.read_csv('test_baseline.csv')
        print(f"  📌 Including baseline data from test_baseline.csv")

    # Extract overlap ratio data from main CSV (2 writers @ 0.1 TPS with overlap > 0)
    overlap_df = df[(df['num_writers'] == 2) &
                    (df['writer_rate'] == 0.1) &
                    (df['key_overlap_ratio'] > 0.0)].copy()
    if len(overlap_df) > 0:
        overlap_df = overlap_df.sort_values('key_overlap_ratio')
        print(f"  📌 Found {len(overlap_df)} overlap ratio configurations in main CSV")

    # Create figure with 4 vertically stacked subplots
    fig, (ax1, ax2, ax3, ax4) = plt.subplots(4, 1, figsize=(20, 20))
    fig.suptitle('Comprehensive Configuration Sweep Results', fontsize=16, fontweight='bold', y=0.995)

    # Subplot 1: Read Latency
    df_read = df.sort_values(['num_readers', 'reader_rate', 'num_writers', 'writer_rate'])
    df_read['config'] = df_read.apply(lambda row: f"R{int(row['num_readers'])}@{int(row['reader_rate'])}_W{int(row['num_writers'])}@{row['writer_rate']:.2f}", axis=1)
    x_read = range(len(df_read))
    ax1.plot(x_read, df_read['read_p50_ms'], 'o-', color='green', label='Reader p50', linewidth=2, markersize=6)
    ax1.plot(x_read, df_read['read_p99_ms'], 's--', color='darkgreen', label='Reader p99', linewidth=2, markersize=6, alpha=0.7)

    if baseline_df is not None and len(baseline_df) > 0:
        baseline_p50 = baseline_df['read_p50_ms'].iloc[0]
        baseline_p99 = baseline_df['read_p99_ms'].iloc[0]
        ax1.axhline(y=baseline_p50, color='green', linestyle=':', linewidth=2, label=f'Baseline p50 ({baseline_p50:.1f}ms)', alpha=0.8)
        ax1.axhline(y=baseline_p99, color='darkgreen', linestyle=':', linewidth=2, label=f'Baseline p99 ({baseline_p99:.1f}ms)', alpha=0.8)

    ax1.set_xticks(x_read[::max(1, len(x_read)//30)])
    ax1.set_xticklabels(df_read['config'].tolist()[::max(1, len(x_read)//30)], rotation=90, ha='right', fontsize=8)
    ax1.set_ylabel('Read Latency (ms)', fontsize=12)
    ax1.set_title('Read Latency vs Configuration', fontsize=14, fontweight='bold')
    ax1.legend(fontsize=11, loc='best')
    ax1.grid(True, alpha=0.3)

    # Subplot 2: Write Latency
    df_write = df.sort_values(['num_writers', 'writer_rate', 'num_readers', 'reader_rate'])
    df_write['config'] = df_write.apply(lambda row: f"W{int(row['num_writers'])}@{row['writer_rate']:.2f}_R{int(row['num_readers'])}@{int(row['reader_rate'])}", axis=1)
    x_write = range(len(df_write))
    ax2.plot(x_write, df_write['write_p50_ms'], 'o-', color='blue', label='Writer p50', linewidth=2, markersize=6)
    ax2.plot(x_write, df_write['write_p99_ms'], 's--', color='darkblue', label='Writer p99', linewidth=2, markersize=6, alpha=0.7)

    if baseline_df is not None and len(baseline_df) > 0:
        baseline_p50 = baseline_df['write_p50_ms'].iloc[0]
        baseline_p99 = baseline_df['write_p99_ms'].iloc[0]
        ax2.axhline(y=baseline_p50, color='blue', linestyle=':', linewidth=2, label=f'Baseline p50 ({baseline_p50:.1f}ms)', alpha=0.8)
        ax2.axhline(y=baseline_p99, color='darkblue', linestyle=':', linewidth=2, label=f'Baseline p99 ({baseline_p99:.1f}ms)', alpha=0.8)

    ax2.set_xticks(x_write[::max(1, len(x_write)//30)])
    ax2.set_xticklabels(df_write['config'].tolist()[::max(1, len(x_write)//30)], rotation=90, ha='right', fontsize=8)
    ax2.set_ylabel('Write Latency (ms)', fontsize=12)
    ax2.set_title('Write Latency vs Configuration', fontsize=14, fontweight='bold')
    ax2.legend(fontsize=11, loc='best')
    ax2.grid(True, alpha=0.3)

    # Subplot 3: Precondition Failure Rate
    grouped = df.groupby(['num_writers', 'writer_rate']).agg({
        'precondition_failure_rate': 'mean'
    }).reset_index()
    grouped = grouped.sort_values(['num_writers', 'writer_rate'])
    grouped['config'] = grouped.apply(lambda row: f"W{int(row['num_writers'])}@{row['writer_rate']:.2f}", axis=1)
    x_precond = range(len(grouped))
    ax3.plot(x_precond, grouped['precondition_failure_rate'], 'o-', color='red', label='Precondition Failure Rate', linewidth=2, markersize=8)

    if baseline_df is not None and len(baseline_df) > 0:
        baseline_rate = baseline_df['precondition_failure_rate'].iloc[0]
        ax3.axhline(y=baseline_rate, color='red', linestyle=':', linewidth=2, label=f'Baseline (W1@0.1): {baseline_rate:.2f}%', alpha=0.8)

    ax3.set_xticks(x_precond)
    ax3.set_xticklabels(grouped['config'].tolist(), rotation=45, ha='right')
    ax3.set_ylabel('Precondition Failure Rate (%)', fontsize=12)
    ax3.set_title('Precondition Failure Rate vs Writer Configuration', fontsize=14, fontweight='bold')
    ax3.legend(fontsize=11, loc='best')
    ax3.grid(True, alpha=0.3)

    # Subplot 4: Overlap Ratio Analysis (embedded in comprehensive CSV)
    if len(overlap_df) > 0:
        # Use overlap ratio sweep data from main CSV
        x_overlap = overlap_df['key_overlap_ratio']
        ax4_twin = ax4.twinx()

        line1 = ax4.plot(x_overlap, overlap_df['precondition_failure_rate'] * 100, 'o-',
                        color='red', label='Precondition Failure Rate', linewidth=2, markersize=8)
        line2 = ax4_twin.plot(x_overlap, overlap_df['retry_success_rate'] * 100, 's-',
                             color='green', label='Retry Success Rate', linewidth=2, markersize=8)

        ax4.axhline(y=10, color='orange', linestyle='--', linewidth=2, alpha=0.7)
        ax4.set_xlabel('Key Overlap Ratio', fontsize=12)
        ax4.set_ylabel('Precondition Failure Rate (%)', fontsize=12, color='red')
        ax4_twin.set_ylabel('Retry Success Rate (%)', fontsize=12, color='green')
        ax4.tick_params(axis='y', labelcolor='red')
        ax4_twin.tick_params(axis='y', labelcolor='green')
        ax4.set_title('Overlap Ratio Impact on Failures and Retries (2 Writers @ 0.1 TPS)', fontsize=14, fontweight='bold')

        lines = line1 + line2 + [plt.Line2D([0], [0], color='orange', linestyle='--', linewidth=2)]
        labels = ['Precondition Failure Rate', 'Retry Success Rate', '10% Target']
        ax4.legend(lines, labels, fontsize=11, loc='upper left')
        ax4.grid(True, alpha=0.3)

        # Annotate sweet spot if found
        overlap_df_copy = overlap_df.copy()
        overlap_df_copy['diff_from_10'] = abs(overlap_df_copy['precondition_failure_rate'] * 100 - 10)
        sweet_spot_idx = overlap_df_copy['diff_from_10'].idxmin()
        sweet_spot = overlap_df_copy.loc[sweet_spot_idx]

        ax4.annotate('Sweet Spot',
                    xy=(sweet_spot['key_overlap_ratio'], sweet_spot['precondition_failure_rate'] * 100),
                    xytext=(10, -30), textcoords='offset points',
                    bbox=dict(boxstyle='round,pad=0.5', fc='yellow', alpha=0.7),
                    arrowprops=dict(arrowstyle='->', connectionstyle='arc3,rad=0', color='black'),
                    fontsize=9, fontweight='bold')
    else:
        # No overlap ratio data in comprehensive sweep
        ax4.text(0.5, 0.5, 'No overlap ratio data found in comprehensive sweep CSV\n(Expected 2 writers @ 0.1 TPS with key_overlap_ratio > 0)',
                ha='center', va='center', fontsize=14, color='gray',
                transform=ax4.transAxes)
        ax4.set_title('Overlap Ratio Analysis (Not Available)', fontsize=14, fontweight='bold')
        ax4.set_xticks([])
        ax4.set_yticks([])

    plt.tight_layout()
    output_file = csv_file.replace('.csv', '.png')
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"\n✅ Comprehensive sweep plot saved to {output_file}")
    plt.close()

def main():
    sns.set_style("whitegrid")

    csv_files = [
        ('test_baseline.csv', 'num_writers', 'Baseline Test Results'),
        ('sweep_num_writers.csv', 'num_writers', 'Precondition Failure vs Number of Writers'),
        ('sweep_writer_tps.csv', 'writer_rate', 'Precondition Failure vs Writer Rate'),
        ('sweep_key_pool.csv', 'key_pool_size', 'Precondition Failure vs Key Pool Size'),
        ('sweep_overlap.csv', 'key_overlap_ratio', 'Precondition Failure vs Key Overlap Ratio'),
    ]

    if len(sys.argv) > 1:
        csv_file = sys.argv[1]

        if len(sys.argv) > 2 and sys.argv[2] == '--comprehensive':
            plot_comprehensive_v2_all(csv_file)
        elif len(sys.argv) > 2 and sys.argv[2] == '--latency':
            plot_latency_sweep(csv_file)
        elif len(sys.argv) > 2 and sys.argv[2] == '--overlap-ratio':
            plot_overlap_ratio_sweep(csv_file)
        elif csv_file == 'sweep_overlap_ratio.csv' or 'overlap_ratio' in csv_file:
            plot_overlap_ratio_sweep(csv_file)
        elif csv_file == 'comprehensive_sweep.csv' or 'comprehensive' in csv_file:
            plot_comprehensive_sweep(csv_file)
        elif csv_file == 'chaos_sweep.csv' or 'chaos' in csv_file or (len(sys.argv) > 2 and sys.argv[2] == '--chaos'):
            plot_chaos_sweep(csv_file)
        else:
            x_col = sys.argv[2] if len(sys.argv) > 2 else 'num_writers'
            title = sys.argv[3] if len(sys.argv) > 3 else 'Test Results'
            plot_sweep(csv_file, x_col, title)
    else:
        print("Plotting all available CSV files...")
        for csv_file, x_col, title in csv_files:
            plot_sweep(csv_file, x_col, title)
        print("\nUsage: python3 plot_results.py [csv_file] [x_column] [title]")
        print("       python3 plot_results.py comprehensive_sweep.csv  # for comprehensive sweep")
        print("       python3 plot_results.py chaos_sweep.csv          # for chaos sweep")

if __name__ == '__main__':
    main()
