use num_derive::FromPrimitive;
use thiserror::Error;


pub type TxResult = Result<(), TxError>;

#[repr(u32)]
#[derive(Error, FromPrimitive, Debug, defmt::Format)]
pub enum TxError {
    #[error("ThreadX error: Deleted")]
    Deleted = threadx_sys::TX_DELETED,
    #[error("ThreadX error: PoolError")]
    PoolError = threadx_sys::TX_POOL_ERROR,
    #[error("ThreadX error: PtrError")]
    PtrError = threadx_sys::TX_PTR_ERROR,
    #[error("ThreadX error: WaitError")]
    WaitError = threadx_sys::TX_WAIT_ERROR,
    #[error("ThreadX error: SizeError")]
    SizeError = threadx_sys::TX_SIZE_ERROR,
    #[error("ThreadX error: GroupError")]
    GroupError = threadx_sys::TX_GROUP_ERROR,
    #[error("ThreadX error: NoEvents")]
    NoEvents = threadx_sys::TX_NO_EVENTS,
    #[error("ThreadX error: OptionError")]
    OptionError = threadx_sys::TX_OPTION_ERROR,
    #[error("ThreadX error: QueueError")]
    QueueError = threadx_sys::TX_QUEUE_ERROR,
    #[error("ThreadX error: QueueEmpty")]
    QueueEmpty = threadx_sys::TX_QUEUE_EMPTY,
    #[error("ThreadX error: QueueFull")]
    QueueFull = threadx_sys::TX_QUEUE_FULL,
    #[error("ThreadX error: SemaphoreError")]
    SemaphoreError = threadx_sys::TX_SEMAPHORE_ERROR,
    #[error("ThreadX error: NoInstance")]
    NoInstance = threadx_sys::TX_NO_INSTANCE,
    #[error("ThreadX error: ThreadError")]
    ThreadError = threadx_sys::TX_THREAD_ERROR,
    #[error("ThreadX error: PriorityError")]
    PriorityError = threadx_sys::TX_PRIORITY_ERROR,
    #[error("ThreadX error: NoMemoryOrStartError")]
    NoMemoryOrStartError = threadx_sys::TX_NO_MEMORY,
    //StartError = threadx_sys::TX_START_ERROR, // threadx has 0x10 defined twice with two different values
    #[error("ThreadX error: DeleteError")]
    DeleteError = threadx_sys::TX_DELETE_ERROR,
    #[error("ThreadX error: ResumeError")]
    ResumeError = threadx_sys::TX_RESUME_ERROR,
    #[error("ThreadX error: CallerError")]
    CallerError = threadx_sys::TX_CALLER_ERROR,
    #[error("ThreadX error: SuspendError")]
    SuspendError = threadx_sys::TX_SUSPEND_ERROR,
    #[error("ThreadX error: TimerError")]
    TimerError = threadx_sys::TX_TIMER_ERROR,
    #[error("ThreadX error: TickError")]
    TickError = threadx_sys::TX_TICK_ERROR,
    #[error("ThreadX error: ActivateError")]
    ActivateError = threadx_sys::TX_ACTIVATE_ERROR,
    #[error("ThreadX error: ThreshError")]
    ThreshError = threadx_sys::TX_THRESH_ERROR,
    #[error("ThreadX error: SuspendLifted")]
    SuspendLifted = threadx_sys::TX_SUSPEND_LIFTED,
    #[error("ThreadX error: WaitAborted")]
    WaitAborted = threadx_sys::TX_WAIT_ABORTED,
    #[error("ThreadX error: WaitAbortError")]
    WaitAbortError = threadx_sys::TX_WAIT_ABORT_ERROR,
    #[error("ThreadX error: MutexError")]
    MutexError = threadx_sys::TX_MUTEX_ERROR,
    #[error("ThreadX error: NotAvailable")]
    NotAvailable = threadx_sys::TX_NOT_AVAILABLE,
    #[error("ThreadX error: NotOwned")]
    NotOwned = threadx_sys::TX_NOT_OWNED,
    #[error("ThreadX error: InheritError")]
    InheritError = threadx_sys::TX_INHERIT_ERROR,
    #[error("ThreadX error: NotDone")]
    NotDone = threadx_sys::TX_NOT_DONE,
    #[error("ThreadX error: CeilingExceeded")]
    CeilingExceeded = threadx_sys::TX_CEILING_EXCEEDED,
    #[error("ThreadX error: InvalidCeiling")]
    InvalidCeiling = threadx_sys::TX_INVALID_CEILING,
    #[error("ThreadX error: FeatureNotEnabled")]
    FeatureNotEnabled = threadx_sys::TX_FEATURE_NOT_ENABLED,
    #[error("ThreadX error: Unknown")]
    Unknown = 0xFE,
}
