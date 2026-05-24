import { test, expect } from '@playwright/test'

const API_BASE = '/api/forge/relay'

/**
 * Helper: create a mock relay run via API (no LLM cost).
 */
async function createMockRun(
  request: any,
  flowId = 'post-discovery',
  steps?: Array<{ id: string; profession_id: string; gate: string }>
) {
  const payload: any = { flow_id: flowId }
  if (steps) payload.steps = steps
  const resp = await request.post(`${API_BASE}/runs`, { data: payload })
  expect(resp.ok()).toBeTruthy()
  const body = await resp.json()
  expect(body.run_id).toBeDefined()
  return body.run_id as string
}

/**
 * Helper: delete a run.
 */
async function deleteRun(request: any, runId: string) {
  await request.delete(`${API_BASE}/runs/${runId}`)
}

test.describe('Relay View', () => {
  test('loads Relay tab and shows run list', async ({ page }) => {
    // Open the app and switch to Relay view
    await page.goto('/forge')
    await page.waitForLoadState('networkidle')

    // Click the Relay (agents) tab in the left rail
    const relayTab = page.locator('[data-testid="nav-rail"] button').filter({ hasText: /Relay|Agents/i }).first()
    await relayTab.click()

    // Wait for the Relay view to render
    await page.waitForSelector('[data-testid="relay-view"]', { timeout: 5000 })

    // Run list sidebar should be visible
    await expect(page.locator('[data-testid="relay-run-list"]')).toBeVisible()
  })

  test('creates a run and displays it in the list', async ({ page, request }) => {
    // Pre-create a run via API so the UI has something to show
    const runId = await createMockRun(request, 'post-discovery')

    await page.goto('/forge')
    await page.waitForLoadState('networkidle')

    // Switch to Relay view
    const relayTab = page.locator('[data-testid="nav-rail"] button').filter({ hasText: /Relay|Agents/i }).first()
    await relayTab.click()

    // The run should appear in the list
    const runCard = page.locator(`[data-testid="run-card-${runId}"]`)
    await expect(runCard).toBeVisible({ timeout: 5000 })

    // Cleanup
    await deleteRun(request, runId)
  })

  test('shows pipeline steps for a selected run', async ({ page, request }) => {
    const runId = await createMockRun(request, 'post-discovery', [
      { id: 'design', profession_id: 'architect', gate: 'auto' },
      { id: 'plan', profession_id: 'planner', gate: 'auto' },
      { id: 'code', profession_id: 'coder', gate: 'auto' },
    ])

    await page.goto('/forge')
    await page.waitForLoadState('networkidle')

    // Switch to Relay view
    const relayTab = page.locator('[data-testid="nav-rail"] button').filter({ hasText: /Relay|Agents/i }).first()
    await relayTab.click()

    // Click the run card
    const runCard = page.locator(`[data-testid="run-card-${runId}"]`)
    await runCard.click()

    // Pipeline steps should render
    await expect(page.locator('[data-testid="pipeline-flow"]')).toBeVisible({ timeout: 5000 })

    // Should show the correct number of steps
    const steps = page.locator('[data-testid="pipeline-step"]')
    await expect(steps).toHaveCount(3)

    await deleteRun(request, runId)
  })

  test('gate panel appears when run waits for human approval', async ({ page, request }) => {
    // Set check mode so all gates are shown in UI
    await page.goto('/forge')
    await page.evaluate(() => { localStorage.setItem('forge-mode', 'check') })

    const runId = await createMockRun(request, 'standard', [
      { id: 'intake', profession_id: 'assistant', gate: 'auto' },
      { id: 'discover', profession_id: 'advisor', gate: 'human' },
    ])

    // Advance to the advisor step and submit a handoff to trigger the gate
    await request.post(`${API_BASE}/runs/${runId}/advance`)
    await request.post(`${API_BASE}/runs/${runId}/handoff`, {
      data: {
        handoff: {
          from: 'assistant',
          to: 'advisor',
          run_id: runId,
          checkpoint_id: 0,
          summary: 'Intake complete.',
          decisions: [],
          open_questions: [],
          spec_updates: [],
          work_product: [],
          context_for_next: { files_to_read: [], specs_to_follow: [], warnings: [] },
          token_usage: { step_input: 100, step_output: 50, cumulative: 150, budget_remaining: 9850 },
        },
      },
    })

    await page.goto('/forge')
    await page.waitForLoadState('networkidle')

    const relayTab = page.locator('[data-testid="nav-rail"] button').filter({ hasText: /Relay|Agents/i }).first()
    await relayTab.click()

    const runCard = page.locator(`[data-testid="run-card-${runId}"]`)
    await runCard.click()

    // Gate panel should appear
    await expect(page.locator('[data-testid="gate-panel"]')).toBeVisible({ timeout: 5000 })

    // Approve the gate
    await page.locator('[data-testid="gate-approve"]').click()

    // Gate panel should disappear
    await expect(page.locator('[data-testid="gate-panel"]')).not.toBeVisible({ timeout: 5000 })

    await deleteRun(request, runId)
  })
})

test.describe('Relay API', () => {
  test('POST /runs creates a run with built-in flow', async ({ request }) => {
    const resp = await request.post(`${API_BASE}/runs`, {
      data: { flow_id: 'goal-discovery', task: 'Test run' },
    })
    expect(resp.ok()).toBeTruthy()
    const body = await resp.json()
    expect(body.run_id).toBeDefined()
    expect(body.state.status).toBe('Idle')
    expect(body.state.total_steps).toBeGreaterThan(0)

    await deleteRun(request, body.run_id)
  })

  test('GET /runs returns list including new run', async ({ request }) => {
    const runId = await createMockRun(request)

    const resp = await request.get(`${API_BASE}/runs`)
    expect(resp.ok()).toBeTruthy()
    const runs = await resp.json()
    expect(Array.isArray(runs)).toBeTruthy()
    expect(runs.some((r: any) => r.run_id === runId)).toBeTruthy()

    await deleteRun(request, runId)
  })

  test('GET /professions returns advisor with write_goals', async ({ request }) => {
    const resp = await request.get(`${API_BASE}/professions`)
    expect(resp.ok()).toBeTruthy()
    const body = await resp.json()
    const advisor = body.professions.find((p: any) => p.id === 'advisor')
    expect(advisor).toBeDefined()
    expect(advisor.allowed_tools).toContain('write_goals')
  })

  test('DELETE /runs removes the run', async ({ request }) => {
    const runId = await createMockRun(request)

    const delResp = await request.delete(`${API_BASE}/runs/${runId}`)
    expect(delResp.status()).toBe(204)

    const getResp = await request.get(`${API_BASE}/runs/${runId}`)
    expect(getResp.status()).toBe(404)
  })
})
