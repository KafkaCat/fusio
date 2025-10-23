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
        'baseline',
        'net-delay-100ms',
        'net-delay-200ms',
        'net-delay-500ms',
        'net-block-10s-3x',
        'cpu-4threads-80pct',
        'combined-200ms-4threads-80pct'
    ]

    if len(df) != len(scenario_labels):
        print(f"⚠️  Expected {len(scenario_labels)} scenarios, got {len(df)}")

    df['scenario'] = scenario_labels[:len(df)]

    print(f"Loaded {len(df)} chaos scenarios from {csv_file}")

    fig = plt.figure(figsize=(20, 12))
    gs = fig.add_gridspec(3, 3, hspace=0.3, wspace=0.3)
    fig.suptitle('Chaos Engineering Test Results', fontsize=16, fontweight='bold')

    axes = [
        [fig.add_subplot(gs[0, 0]), fig.add_subplot(gs[0, 1]), fig.add_subplot(gs[0, 2])],
        [fig.add_subplot(gs[1, 0]), fig.add_subplot(gs[1, 1]), fig.add_subplot(gs[1, 2])],
        [fig.add_subplot(gs[2, :])]
    ]

    baseline_failure_rate = df.iloc[0]['precondition_failure_rate']
    baseline_tps = df.iloc[0]['write_tps']
    baseline_p99 = df.iloc[0]['write_p99_ms']
    baseline_read_p99 = df.iloc[0]['read_p99_ms']

    ax1 = axes[0][0]
    colors = ['green' if i == 0 else 'orange' if 'net' in s else 'red' if 'cpu' in s else 'purple'
              for i, s in enumerate(df['scenario'])]
    bars = ax1.bar(range(len(df)), df['precondition_failure_rate'] * 100, color=colors)
    ax1.axhline(y=baseline_failure_rate * 100, color='green', linestyle='--', linewidth=2, label='Baseline')
    ax1.set_xticks(range(len(df)))
    ax1.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax1.set_ylabel('Precondition Failure Rate (%)', fontsize=10)
    ax1.set_title('Failure Rate Under Chaos', fontsize=11, fontweight='bold')
    ax1.legend()
    ax1.grid(True, axis='y', alpha=0.3)

    ax2 = axes[0][1]
    bars = ax2.bar(range(len(df)), df['write_tps'], color=colors)
    ax2.axhline(y=baseline_tps, color='green', linestyle='--', linewidth=2, label='Baseline')
    ax2.set_xticks(range(len(df)))
    ax2.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax2.set_ylabel('Write TPS', fontsize=10)
    ax2.set_title('Write Throughput Under Chaos', fontsize=11, fontweight='bold')
    ax2.legend()
    ax2.grid(True, axis='y', alpha=0.3)

    ax3 = axes[0][2]
    ax3.plot(df['scenario'], df['write_p50_ms'], marker='o', label='p50', linewidth=2)
    ax3.plot(df['scenario'], df['write_p95_ms'], marker='s', label='p95', linewidth=2)
    ax3.plot(df['scenario'], df['write_p99_ms'], marker='^', label='p99', linewidth=2)
    ax3.axhline(y=baseline_p99, color='green', linestyle='--', linewidth=2, alpha=0.5)
    ax3.set_xticks(range(len(df)))
    ax3.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax3.set_ylabel('Write Latency (ms)', fontsize=10)
    ax3.set_title('Write Latency Distribution Under Chaos', fontsize=11, fontweight='bold')
    ax3.legend()
    ax3.grid(True, alpha=0.3)

    ax4 = axes[1][0]
    degradation_pct = ((df['precondition_failure_rate'] - baseline_failure_rate) / baseline_failure_rate * 100)
    bars = ax4.bar(range(len(df)), degradation_pct, color=colors)
    ax4.axhline(y=0, color='black', linestyle='-', linewidth=1)
    ax4.set_xticks(range(len(df)))
    ax4.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax4.set_ylabel('Degradation (%)', fontsize=10)
    ax4.set_title('Failure Rate Degradation vs Baseline', fontsize=11, fontweight='bold')
    ax4.grid(True, axis='y', alpha=0.3)

    ax5 = axes[1][1]
    tps_degradation_pct = ((df['write_tps'] - baseline_tps) / baseline_tps * 100)
    bars = ax5.bar(range(len(df)), tps_degradation_pct, color=colors)
    ax5.axhline(y=0, color='black', linestyle='-', linewidth=1)
    ax5.set_xticks(range(len(df)))
    ax5.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax5.set_ylabel('Degradation (%)', fontsize=10)
    ax5.set_title('TPS Degradation vs Baseline', fontsize=11, fontweight='bold')
    ax5.grid(True, axis='y', alpha=0.3)

    ax6 = axes[1][2]
    ax6.plot(df['scenario'], df['read_p50_ms'], marker='o', label='p50', linewidth=2)
    ax6.plot(df['scenario'], df['read_p99_ms'], marker='^', label='p99', linewidth=2)
    ax6.axhline(y=baseline_read_p99, color='green', linestyle='--', linewidth=2, alpha=0.5, label='Baseline p99')
    ax6.set_xticks(range(len(df)))
    ax6.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax6.set_ylabel('Read Latency (ms)', fontsize=10)
    ax6.set_title('Read Latency Distribution Under Chaos', fontsize=11, fontweight='bold')
    ax6.legend()
    ax6.grid(True, alpha=0.3)

    ax7 = axes[2][0]
    x = range(len(df))
    width = 0.35
    ax7.bar([i - width/2 for i in x], df['retry_success_rate'] * 100, width, label='Retry Success Rate', color='green', alpha=0.7)
    ax7.bar([i + width/2 for i in x], df['retry_failure_rate'] * 100, width, label='Retry Failure Rate', color='red', alpha=0.7)
    ax7.set_xticks(x)
    ax7.set_xticklabels(df['scenario'], rotation=45, ha='right', fontsize=8)
    ax7.set_ylabel('Rate (%)', fontsize=10)
    ax7.set_title('Retry Effectiveness Analysis', fontsize=11, fontweight='bold')
    ax7.legend()
    ax7.grid(True, axis='y', alpha=0.3)

    plt.tight_layout()
    output_file = f'{csv_file[:-4]}.png'
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"✅ Chaos plot saved to {output_file}")
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

        if csv_file == 'comprehensive_sweep.csv' or 'comprehensive' in csv_file:
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
