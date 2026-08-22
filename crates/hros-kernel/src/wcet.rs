//! WCET ledger + RTA proof (Phase 4, WCEF.md)
//! E(Ti) = T_JIT(S) + T_Exec(P) + T_Cap + T_Ctx
//! R_i^{(k+1)} = W_Ti + Σ ceil(R_i^{(k)}/P_j) * W_Tj

/// JIT bound: T_JIT(S) = S * C_lexer, C_lexer ≤25 cyc/B, S ≤256
#[inline(always)]
pub fn t_jit(s_bytes: usize) -> usize {
    s_bytes * 25
}

/// Capability bound: N accesses * C_guard (3c scalar, 1c vector)
#[inline(always)]
pub fn t_cap(n_accesses: usize, vector: bool) -> usize {
    n_accesses * if vector { 1 } else { 3 }
}

/// Context switch bound: 43c (12 auto +8 push +3 sched +8 pop +12 unstack)
pub const T_CTX: usize = 43;

/// Execution bound: Σ BB_cost * bound
#[inline(always)]
pub fn t_exec(bb_costs: &[usize], bounds: &[usize]) -> usize {
    bb_costs.iter().zip(bounds.iter()).map(|(c, b)| c * b).sum()
}

/// Total WCET E(Ti) = T_JIT(S) + T_Cap + T_Exec + T_Ctx
#[inline(always)]
pub fn total_wcet(
    s_bytes: usize,
    n_cap: usize,
    vector: bool,
    bb_costs: &[usize],
    bounds: &[usize],
) -> usize {
    t_jit(s_bytes) + t_cap(n_cap, vector) + t_exec(bb_costs, bounds) + T_CTX
}

/// RTA: R_i^{(k+1)} = W_Ti + Σ ceil(R_i^{(k)}/P_j) * W_Tj
/// Returns Some(R_i) if schedulable (R_i ≤ D_i), None if not.
pub fn rta_response_time(w_ti: usize, higher: &[(usize, usize)], deadline: usize) -> Option<usize> {
    // higher: (W_Tj, P_j)
    let mut r = w_ti;
    for _ in 0..100 {
        let mut next = w_ti;
        for (w_tj, p_j) in higher {
            next += ((r + p_j - 1) / p_j) * w_tj;
        }
        if next > deadline {
            return None;
        }
        if next == r {
            return Some(r);
        }
        r = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wcet_ledger() {
        // Example: S=64B, N=10, vector, 2 BBs
        let e = total_wcet(64, 10, true, &[10, 20], &[5, 3]);
        // T_JIT 64*25=1600, T_Cap 10*1=10, T_Exec 10*5+20*3=110, T_Ctx 43 => 1763
        assert_eq!(e, 1600 + 10 + 110 + 43);
    }

    #[test]
    fn rta_schedulable() {
        // 3 tasks: T1 W=10 P=50 D=50, T2 W=20 P=100 D=100, T3 W=30 P=200 D=200
        // T1: R=10 ≤50 pass
        // T2: R=20 + ceil(20/50)*10=30 ≤100 pass
        // T3: R=30 + ceil(30/50)*10 + ceil(30/100)*20=60 -> 70 after 2nd iter ≤200 pass
        assert_eq!(rta_response_time(10, &[], 50), Some(10));
        assert_eq!(rta_response_time(20, &[(10, 50)], 100), Some(30));
        assert_eq!(rta_response_time(30, &[(10, 50), (20, 100)], 200), Some(70));
    }

    #[test]
    fn rta_unschedulable() {
        // T1 W=40 P=50, T2 W=20 P=100 D=50 -> T2 needs 20+40=60 >50
        assert_eq!(rta_response_time(20, &[(40, 50)], 50), None);
    }
}
